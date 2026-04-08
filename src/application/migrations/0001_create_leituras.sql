CREATE TABLE IF NOT EXISTS leituras (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    temperatura REAL    NOT NULL,
    umidade     REAL    NOT NULL,
    pressao     REAL,
    localizacao TEXT    DEFAULT 'Lab',
    timestamp   DATETIME DEFAULT (datetime('now', 'localtime'))
);
