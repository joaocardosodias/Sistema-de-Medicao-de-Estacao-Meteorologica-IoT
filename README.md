# 🌦️ Sistema de Medição de Estação Meteorológica IoT

Sistema completo de ponta a ponta para coleta, persistência e visualização de dados meteorológicos via IoT.

---

## 📐 Decisão de Arquitetura

A atividade sugeria **Arduino + Python + Flask + Serial USB**. Este projeto optou por uma arquitetura diferente, documentada abaixo:

| Camada | Sugerido | Implementado | Justificativa |
|---|---|---|---|
| Dispositivo | Arduino Uno + DHT11/BMP180 | **ESP32 + DHT11 + BME280** | ESP32 possui Wi-Fi nativo, eliminando a necessidade de cabo USB e script de leitura serial |
| Comunicação | Serial USB → script Python | **MQTT (broker.hivemq.com)** | Protocolo padrão IoT, assíncrono e sem fio |
| Backend | Python + Flask | **Rust + actix-web + sqlx** | Performance superior, tipagem estática, sem runtime overhead |
| Frontend | Jinja2 (SSR) | **HTML + CSS + JS puro (CSR)** | Single-page experience sem recarregar a página; API JSON pura |
| Banco | SQLite direto | **SQLite + sqlx migrate** | Migrations versionadas no repositório |

**Simulação:** O ESP32 lê sensores reais (DHT11 e BME280). Caso os sensores não estejam disponíveis, o script `backend/seed.py` insere 50 leituras mockadas diretamente no banco.

---

## 🗂️ Estrutura do Repositório

```
.
├── backend/                    # Servidor Web/API em Rust
│   ├── src/
│   │   ├── main.rs             # Entry point — registra rotas e serve arquivos estáticos
│   │   ├── api.rs              # Handlers HTTP (API JSON pura)
│   │   ├── db.rs               # Queries SQLite (CRUD + paginação + gráfico)
│   │   ├── models.rs           # Structs Leitura, NovaLeitura, Estatisticas
│   │   └── mqtt.rs             # Cliente MQTT (subscreve esp32/sensores)
│   ├── migrations/
│   │   └── 0001_create_leituras.sql   # Migration da tabela principal
│   ├── static/                 # Frontend CSR (servido como arquivos estáticos)
│   │   ├── index.html          # Dashboard com Chart.js e auto-refresh
│   │   ├── historico.html      # Histórico paginado com exclusão e edição
│   │   ├── editar.html         # Formulário de edição via PUT
│   │   └── css/style.css       # Design system dark mode
│   ├── seed.py                 # Insere 50 leituras mockadas no banco
│   └── Cargo.toml
├── esp_code/                   # Firmware Rust para ESP32
│   └── src/main.rs             # Lê DHT11 + BME280 e publica via MQTT
├── docs/
│   └── atividade_estacao_meteorologica.pdf
└── README.md
```

---

## 🚀 Instalação e Execução

### Pré-requisitos

- [Rust](https://rustup.rs/) — `rustup update`
- [sqlx-cli](https://github.com/launchbadge/sqlx/tree/main/sqlx-cli): `cargo install sqlx-cli --no-default-features --features sqlite`
- Python 3.10+ (para o seed de dados mockados)

### 1. Clonar e entrar no backend

```bash
git clone https://github.com/joaocardosodias/Sistema-de-Medicao-de-Estacao-Meteorologica-IoT.git
cd Sistema-de-Medicao-de-Estacao-Meteorologica-IoT/backend
```

### 2. Criar o banco e rodar as migrations

```bash
export DATABASE_URL="sqlite:dados.db?mode=rwc"
sqlx migrate run
```

### 3. (Opcional) Popular com dados mockados

```bash
python3 seed.py
```

### 4. Rodar o servidor

```bash
cargo run
```

Acesse: **http://localhost:8080**

---

## 📡 Rotas da API

Todas as rotas retornam **JSON**. Base: `http://localhost:8080`

| Método | Rota | Descrição |
|---|---|---|
| `GET` | `/api/leituras?limit=20&offset=0` | Lista paginada de leituras |
| `POST` | `/api/leituras` | Cria nova leitura |
| `GET` | `/api/leituras/{id}` | Busca leitura por ID |
| `PUT` | `/api/leituras/{id}` | Atualiza leitura |
| `DELETE` | `/api/leituras/{id}` | Remove leitura |
| `GET` | `/api/estatisticas` | Média, mínima e máxima |
| `GET` | `/api/grafico?limit=20` | Série temporal para o gráfico |

### Exemplo de payload (POST/PUT)

```json
{
  "temperatura": 25.5,
  "umidade": 65.0,
  "pressao": 1013.2,
  "localizacao": "Lab"
}
```

---

## 🔌 Integração MQTT (ESP32)

- **Broker:** `broker.hivemq.com:1883`
- **Tópico:** `esp32/sensores`
- **Payload:** `{"temperatura": 25.5, "umidade": 65.0, "pressao": 1013.2}`

O backend assina automaticamente o tópico ao iniciar e persiste cada mensagem no banco.

---

## ✅ Critérios de Avaliação

| Critério | Status |
|---|---|
| Comunicação Dispositivo → API | ✅ ESP32 → MQTT → Backend |
| API REST completa | ✅ GET, POST, PUT, DELETE + estatísticas |
| Banco de dados com schema correto | ✅ Migration versionada + CRUD |
| Interface Web funcional | ✅ Dashboard, Histórico, Edição — CSR sem reload |
| Gráfico de variação temporal | ✅ Chart.js (temperatura + umidade) |
| README com instruções | ✅ Este documento |
| Organização e boas práticas | ✅ Módulos separados, migrations, CSS externo |