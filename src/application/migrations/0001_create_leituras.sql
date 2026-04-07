-- Migration: 0001_create_leituras
-- Criação da tabela principal de leituras da estação meteorológica

CREATE TABLE IF NOT EXISTS leituras (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    temperatura REAL    NOT NULL,
    umidade     REAL    NOT NULL,
    pressao     REAL,
    localizacao TEXT    DEFAULT 'Lab',
    timestamp   DATETIME DEFAULT (datetime('now', 'localtime'))
);
