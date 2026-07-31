# Диаграммы Mermaid

## Flowchart

```mermaid
graph TD
    A[Начало] --> B{Всё ок?}
    B -->|да| C[Продолжаем]
    B -->|нет| D((Стоп))
    C --> E(Конец)
    D --> E
```

```mermaid
flowchart LR
    alpha --> beta --> gamma
```

```mermaid
graph TD
    A -->|очень длинная подпись ребра, которая не должна уезжать за пределы страницы, а переносится по ширине и резервирует себе место между узлами диаграммы| B
```

## Sequence

```mermaid
sequenceDiagram
    participant C as Клиент
    participant S as Сервер
    C->>S: запрос
    S-->C: ответ
```

## Subgraph, cylinder, note, alt

```mermaid
graph LR
    Client["Клиент<br/>снаружи"] -->|HTTPS| API
    subgraph Server["Сервер"]
        API[API] --> DB[(PostgreSQL)]
        API --> Files[/файлы/]
    end
```

```mermaid
sequenceDiagram
    participant App as Mini App
    participant API as API
    App->>API: login
    Note over API: проверить подпись
    alt ok
        API-->>App: 200
    else
        API-->>App: 401
    end
```

## Деградация до кода

Нераспознанный тип диаграммы.

```mermaid
totallyNotADiagram
A --> B
```

Внешняя ссылка в выходном SVG запрещена политикой ресурсов (ТЗ §33.3),
поэтому такая диаграмма тоже деградирует до кода.

```mermaid
flowchart TD
    A[Ссылка] --> B[Конец]
    click A "https://example.com"
```
