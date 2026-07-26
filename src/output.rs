//! Запись результата (ТЗ §6.4).
//!
//! PDF полностью формируется в памяти, затем: временный файл в каталоге
//! назначения → запись → flush → переименование. При любой ошибке существующий
//! выходной файл остаётся нетронутым, а временный удаляется.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::AppError;

/// Путь PDF по умолчанию: `input.md` → `input.pdf` (ТЗ §5.1).
#[must_use]
pub fn default_output_path(input: &Path) -> PathBuf {
    input.with_extension("pdf")
}

/// Атомарно записывает PDF.
///
/// # Errors
///
/// [`AppError::OutputExists`], если файл уже существует и не задан
/// `--overwrite` (ТЗ §6.2); [`AppError::Output`] при ошибке ввода-вывода.
pub fn write_pdf_atomically(path: &Path, bytes: &[u8], overwrite: bool) -> Result<(), AppError> {
    // Имя занимается атомарно: проверка `exists()` с последующей записью
    // оставляла бы окно, в котором два параллельных запуска оба считают файл
    // отсутствующим и второй молча затирает результат первого (ТЗ §6.2).
    let reserved = if overwrite {
        false
    } else {
        match File::create_new(path) {
            Ok(_) => true,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(AppError::OutputExists {
                    path: path.to_path_buf(),
                });
            }
            Err(source) => {
                return Err(AppError::Output {
                    path: path.to_path_buf(),
                    source,
                });
            }
        }
    };

    // Резервация — наш собственный пустой файл, при неудаче его нужно убрать,
    // иначе на диске останется пустышка вместо отсутствующего результата.
    let cleanup = |error: std::io::Error, failed: &Path| {
        if reserved {
            let _ = fs::remove_file(path);
        }
        AppError::Output {
            path: failed.to_path_buf(),
            source: error,
        }
    };

    let temporary = temporary_path(path);
    if let Err(error) = write_all(&temporary, bytes) {
        // Незавершённый временный файл после себя не оставляем.
        let _ = fs::remove_file(&temporary);
        return Err(cleanup(error, &temporary));
    }

    replace(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        cleanup(error, path)
    })
}

/// Временный файл создаётся рядом с целевым: переименование обязано остаться
/// в пределах одной файловой системы (ТЗ §6.4).
fn temporary_path(path: &Path) -> PathBuf {
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let name = path
        .file_name()
        .map_or_else(|| "output".to_owned(), |name| name.to_string_lossy().into());
    // Идентификатор процесса разводит параллельные запуски в один каталог.
    directory.join(format!(".{name}.{}.tmp", std::process::id()))
}

fn write_all(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    // Данные должны лежать на диске до переименования, иначе после сбоя
    // питания получится пустой, но «успешно созданный» PDF.
    file.sync_all()
}

/// Переименование поверх существующего файла.
///
/// На Unix `rename` заменяет цель атомарно и этим всё заканчивается. Windows
/// на существующей цели возвращает ошибку, поэтому старый файл сначала
/// уводится в резервную копию — **не удаляется**: если второе переименование
/// сорвётся, прежний PDF возвращается на место. Удалять цель сразу значило бы
/// терять результат при любом сбое второго шага.
fn replace(temporary: &Path, target: &Path) -> std::io::Result<()> {
    match fs::rename(temporary, target) {
        Ok(()) => return Ok(()),
        // Цели нет — значит дело не в ней, ошибку возвращаем как есть.
        Err(error) if !target.exists() => return Err(error),
        Err(_) => {}
    }

    let backup = backup_path(target);
    fs::rename(target, &backup)?;

    match fs::rename(temporary, target) {
        Ok(()) => {
            let _ = fs::remove_file(&backup);
            Ok(())
        }
        Err(error) => {
            // Возвращаем прежний файл на место: вызывающая сторона получит
            // ошибку, но не потерю результата.
            let _ = fs::rename(&backup, target);
            Err(error)
        }
    }
}

/// Имя резервной копии рядом с целью — переименование не должно уходить
/// на другую файловую систему.
fn backup_path(target: &Path) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.backup", std::process::id()));
    target.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PDF: &[u8] = b"%PDF-1.7\n...";

    fn leftovers(directory: &Path) -> Vec<String> {
        fs::read_dir(directory)
            .expect("read dir")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect()
    }

    #[test]
    fn extension_is_replaced() {
        assert_eq!(
            default_output_path(Path::new("docs/input.md")),
            PathBuf::from("docs/input.pdf")
        );
    }

    #[test]
    fn missing_extension_gets_pdf() {
        assert_eq!(
            default_output_path(Path::new("README")),
            PathBuf::from("README.pdf")
        );
    }

    #[test]
    fn dots_in_name_are_preserved() {
        assert_eq!(
            default_output_path(Path::new("v1.2.notes.md")),
            PathBuf::from("v1.2.notes.pdf")
        );
    }

    #[test]
    fn a_new_file_is_written() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("out.pdf");

        write_pdf_atomically(&path, PDF, false).expect("writes");

        assert_eq!(fs::read(&path).expect("read back"), PDF);
        assert!(leftovers(dir.path()).is_empty(), "temporary file remained");
    }

    #[test]
    fn an_existing_file_is_kept_without_overwrite() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("out.pdf");
        fs::write(&path, b"old").expect("seed");

        let err = write_pdf_atomically(&path, PDF, false).expect_err("must refuse");

        assert!(matches!(err, AppError::OutputExists { .. }));
        assert_eq!(fs::read(&path).expect("read back"), b"old");
        assert!(leftovers(dir.path()).is_empty());
    }

    #[test]
    fn an_existing_file_is_replaced_with_overwrite() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("out.pdf");
        fs::write(&path, b"old").expect("seed");

        write_pdf_atomically(&path, PDF, true).expect("writes");

        assert_eq!(fs::read(&path).expect("read back"), PDF);
        assert!(leftovers(dir.path()).is_empty());
    }

    #[test]
    fn an_unwritable_directory_leaves_nothing_behind() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("missing").join("out.pdf");

        let err = write_pdf_atomically(&path, PDF, false).expect_err("must fail");

        assert!(matches!(err, AppError::Output { .. }));
        assert!(!path.exists());
        assert!(leftovers(dir.path()).is_empty());
    }

    #[test]
    fn a_failed_replacement_puts_the_old_file_back() {
        let dir = tempfile::tempdir().expect("temp dir");
        let target = dir.path().join("out.pdf");
        fs::write(&target, b"old").expect("seed");

        // Временного файла нет, поэтому второе переименование заведомо
        // сорвётся — ровно тот случай, когда прежний PDF нельзя терять.
        let missing = dir.path().join("missing.tmp");
        let error = replace(&missing, &target).expect_err("replacement must fail");
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);

        assert_eq!(fs::read(&target).expect("read back"), b"old");
        let backups: Vec<String> = fs::read_dir(dir.path())
            .expect("read dir")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".backup"))
            .collect();
        assert!(backups.is_empty(), "backup remained: {backups:?}");
    }

    #[test]
    fn only_one_of_two_concurrent_writers_succeeds() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("out.pdf");

        // Проверка «файл существует» с последующей записью допускала гонку:
        // оба запуска считали файл отсутствующим и второй затирал первый.
        let outcomes: Vec<bool> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..2)
                .map(|_| scope.spawn(|| write_pdf_atomically(&path, PDF, false).is_ok()))
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap_or(false))
                .collect()
        });

        assert_eq!(
            outcomes.iter().filter(|success| **success).count(),
            1,
            "exactly one writer must win without --overwrite"
        );
        assert_eq!(fs::read(&path).expect("read back"), PDF);
        assert!(leftovers(dir.path()).is_empty());
    }

    #[test]
    fn a_reserved_name_is_released_when_writing_fails() {
        let dir = tempfile::tempdir().expect("temp dir");
        // Каталог вместо файла: запись во временный файл внутри него невозможна.
        let path = dir.path().join("sub").join("out.pdf");

        let err = write_pdf_atomically(&path, PDF, false).expect_err("must fail");

        assert!(matches!(err, AppError::Output { .. }));
        assert!(!path.exists(), "reserved empty file remained");
    }

    #[test]
    fn the_temporary_file_sits_next_to_the_target() {
        let temporary = temporary_path(Path::new("docs/out.pdf"));
        assert_eq!(temporary.parent(), Some(Path::new("docs")));
        assert!(
            temporary
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".out.pdf.") && name.ends_with(".tmp"))
        );
    }

    #[test]
    fn a_bare_file_name_writes_into_the_current_directory() {
        assert_eq!(
            temporary_path(Path::new("out.pdf")).parent(),
            Some(Path::new("."))
        );
    }
}
