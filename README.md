# Sistema de Medicao de Estacao Meteorologica IoT

Sistema completo de ponta a ponta para coleta, persistencia e visualizacao de dados meteorologicos via IoT. Um ESP32 coleta dados de temperatura, umidade e pressao atmosferica e os transmite via MQTT para um servidor Rust que os armazena e os expoe atraves de uma API REST com dashboard web em tempo real.

---

## Decisao de Arquitetura

A atividade sugeria **Arduino Uno + Python + Flask + Serial USB**. Este projeto optou por uma arquitetura diferente, com justificativas tecnicas documentadas abaixo.

| Camada | Sugerido | Implementado | Justificativa |
|---|---|---|---|
| Dispositivo | Arduino Uno + DHT11/BMP180 | ESP32 + DHT11 + BME280 | ESP32 possui Wi-Fi nativo, eliminando cabo USB e script de leitura serial |
| Comunicacao | Serial USB → script Python | MQTT (broker local) | Protocolo padrao IoT, assíncrono, sem fio, com reconexao automatica |
| Backend | Python + Flask | Rust + actix-web + sqlx | Performance superior, tipagem estatica, sem GIL, sem runtime overhead |
| Frontend | Jinja2 (SSR) | HTML + CSS + JS puro (CSR) | Single-page sem recarregar a pagina; API JSON desacoplada do servidor |
| Banco | SQLite direto | SQLite + sqlx migrate | Migrations versionadas no repositorio, aplicadas automaticamente na subida |

A substituicao do Arduino pelo ESP32 elimina a necessidade do `serial_reader.py` — o proprio dispositivo publica dados via Wi-Fi. O backend Rust assina o broker MQTT e persiste cada mensagem, cumprindo o mesmo papel de "bridge" que o script serial faria.

---

### Por que Rust no Backend?

1. **Performance:** O `actix-web` esta entre os frameworks web mais rapidos disponiveis, servindo dezenas de milhares de requisicoes por segundo sem precisar de multiplos workers.

2. **Seguranca de memoria em tempo de compilacao:** O compilador garante ausencia de null pointer dereferences, data races e use-after-free — erros comuns em Python e C.

3. **Tipagem forte com `serde` e `sqlx`:** Cada campo JSON e cada query SQL sao verificados em tempo de compilacao. Uma mudanca de schema que quebre o codigo gera erro de compilacao, nao crash em producao.

4. **Concorrencia real com `tokio`:** O runtime assincrono permite atender requisicoes HTTP e consumir mensagens MQTT simultaneamente na mesma thread, sem callbacks ou threads bloqueantes adicionais.

5. **Binario unico:** O `cargo build --release` gera um unico executavel estatico. Sem Python instalado, sem virtualenv, sem pip em producao.

---

### Por que Rust no Firmware (ESP32)?

1. **Abstrações seguras sobre hardware:** O `esp-idf-hal` encapsula GPIOs, I2C e Wi-Fi com tipos seguros. O firmware opera sensores fisicos e MQTT sem escrever codigo `unsafe`.

2. **Integracao com ESP-IDF:** O `esp-idf-sys` compila o SDK oficial da Espressif como dependencia. O firmware Rust roda sobre o FreeRTOS do ESP-IDF sem necessidade de configuracao manual.

3. **Tratamento de erros explicito:** Com `anyhow::Result` e o operador `?`, cada ponto de falha (Wi-Fi, sensor, MQTT) e tratado explicitamente — sem excecoes silenciosas.

4. **Backoff exponencial:** O loop principal implementa retry com backoff exponencial (2^n segundos, maximo 30s) em caso de falha do sensor.

5. **Toolchain automatizada pelo `embuild`:** A biblioteca `embuild` baixa e configura automaticamente o compilador `xtensa-esp32-elf-gcc` e o ESP-IDF. Um simples `cargo build` e suficiente na primeira vez.

---

## Estrutura do Repositorio

```
.
├── src/
│   ├── application/                     # Servidor Web/API em Rust
│   │   ├── src/
│   │   │   ├── main.rs                  # Entry point: inicializa banco, MQTT e servidor HTTP
│   │   │   ├── api.rs                   # Handlers HTTP da API REST
│   │   │   ├── db.rs                    # Camada de acesso a dados: CRUD, paginacao, grafico
│   │   │   ├── models.rs                # Structs: Leitura, NovaLeitura, Estatisticas
│   │   │   ├── mqtt.rs                  # Cliente MQTT assíncrono (subscreve esp32/sensores)
│   │   │   └── simulador.rs             # Binario auxiliar: publica dados simulados via MQTT
│   │   ├── migrations/
│   │   │   └── 0001_create_leituras.sql # Schema do banco (aplicado automaticamente na subida)
│   │   ├── static/
│   │   │   ├── index.html               # Dashboard com Chart.js e atualizacao via WebSocket
│   │   │   ├── historico.html           # Historico paginado com exclusao e edicao
│   │   │   ├── editar.html              # Formulario de edicao via PUT
│   │   │   └── css/style.css            # Design system dark mode
│   │   ├── dados.db                     # Banco SQLite com leituras de exemplo (141 registros)
│   │   └── Cargo.toml
│   │
│   └── esp_code/                        # Firmware Rust para ESP32
│       ├── src/
│       │   └── main.rs                  # Le DHT11 + BME280 e publica JSON via MQTT
│       ├── .cargo/
│       │   └── config.toml              # Target xtensa-esp32-espidf, runner espflash, versao ESP-IDF
│       ├── build.rs                     # Invoca embuild para configurar o ESP-IDF
│       ├── rust-toolchain.toml          # Fixa o toolchain Rust com suporte Xtensa
│       └── Cargo.toml
│
├── .embuild/                            # Gerado automaticamente na primeira compilacao do firmware
├── Cargo.toml                           # Workspace raiz
└── README.md
```

> A pasta `.embuild/` e gerada automaticamente pelo `embuild` na primeira execucao de `cargo build` dentro de `src/esp_code/`. Ela contem o compilador Xtensa, o ESP-IDF e ferramentas auxiliares (aproximadamente 1 GB). Ela esta no `.gitignore` e nao deve ser commitada. Se deletada, rode `cargo clean && cargo build` dentro de `src/esp_code/` para regenera-la.

---

## Execucao

O projeto tem duas partes independentes: o servidor backend e o firmware do ESP32.

---

### Parte 1 — Backend (Servidor Web + API)

#### Pre-requisitos

- [Rust](https://rustup.rs/) — `rustup update` para garantir versao recente
- `sqlx-cli` para gerenciar as migrations:
  ```bash
  cargo install sqlx-cli --no-default-features --features sqlite
  ```
- Um broker MQTT local (ex: [Mosquitto](https://mosquitto.org/)) rodando em `localhost:1883`

#### Passo a passo

```bash
# Entrar na pasta do servidor
cd src/application

# Criar o banco e aplicar as migrations
export DATABASE_URL="sqlite:dados.db?mode=rwc"
sqlx migrate run

# Iniciar o servidor
cargo run --bin servidor
```

O servidor estara disponivel em `http://localhost:8080`.

Ao subir, dois servicos sao iniciados simultaneamente:
- **API REST** na porta 8080 (rotas `/api/...`)
- **Cliente MQTT** que assina o topico `esp32/sensores` e persiste cada mensagem no banco

---

#### Simulacao sem hardware

Caso o ESP32 nao esteja disponivel, use o simulador incluso. Ele publica leituras aleatorias realistas no mesmo topico MQTT que o firmware real usaria:

```bash
# Em outro terminal, com o servidor ja rodando
cd src/application
cargo run --bin simulador
```

Variaveis de ambiente opcionais para o simulador:

| Variavel | Padrao | Descricao |
|---|---|---|
| `MQTT_BROKER` | `localhost` | Host do broker |
| `MQTT_PORT` | `1883` | Porta do broker |
| `MQTT_TOPIC` | `esp32/sensores` | Topico de publicacao |
| `INTERVAL_SECS` | `5` | Intervalo entre leituras |

---

### Parte 2 — Firmware ESP32

#### Pre-requisitos

- [Rust](https://rustup.rs/)
- `espflash` para gravar o firmware na placa:
  ```bash
  cargo install espflash
  ```
- `ldproxy`:
  ```bash
  cargo install ldproxy
  ```

> Na primeira compilacao, o `embuild` baixara o ESP-IDF e o compilador Xtensa automaticamente. Isso pode levar 10 a 20 minutos e requer aproximadamente 1 GB de espaco em disco. As compilacoes seguintes usam o cache local e sao rapidas.

> Se a pasta `.embuild/` for deletada manualmente, execute `cargo clean` antes de `cargo build` para forcara regeneracao completa.

#### Configuracao

Edite as constantes no topo de `src/esp_code/src/main.rs`:

```rust
const WIFI_SSID:   &str = "NomeDaSuaRede";
const WIFI_PASS:   &str = "SenhaDaRede";
const MQTT_BROKER: &str = "mqtt://IP_DO_SERVIDOR:1883";
```

Ou use variaveis de ambiente em tempo de compilacao:

```bash
export CONFIG_WIFI_SSID="MinhaRede"
export CONFIG_WIFI_PASS="MinhaSenha"
export CONFIG_MQTT_BROKER_URL="mqtt://192.168.1.100:1883"
```

#### Compilar e gravar

```bash
cd src/esp_code
cargo run
```

O comando compila o firmware, grava na placa via `espflash flash` e abre o monitor serial `--monitor` para acompanhar os logs em tempo real.

#### Conexao de hardware

| Sensor | Pino ESP32 | Observacao |
|---|---|---|
| DHT11 — Data | GPIO4 | Resistor pull-up de 4.7 kOhm entre Data e 3.3V recomendado |
| BME280 — SDA | GPIO21 | Barramento I2C padrao do ESP32 |
| BME280 — SCL | GPIO22 | Barramento I2C padrao do ESP32 |
| VCC (ambos) | 3.3V | |
| GND (ambos) | GND | |

O BME280 e opcional. Se nao detectado, o firmware publica apenas temperatura e umidade (DHT11), omitindo o campo `pressao`.

---

## API REST

Todas as rotas retornam JSON. Base URL: `http://localhost:8080`

| Metodo | Rota | Descricao |
|---|---|---|
| `GET` | `/api/leituras?limit=20&offset=0` | Lista paginada de leituras |
| `POST` | `/api/leituras` | Cria nova leitura |
| `GET` | `/api/leituras/{id}` | Busca leitura por ID |
| `PUT` | `/api/leituras/{id}` | Atualiza uma leitura |
| `DELETE` | `/api/leituras/{id}` | Remove uma leitura |
| `GET` | `/api/estatisticas` | Media, minima e maxima de temperatura e umidade |
| `GET` | `/api/grafico?limit=20` | Serie temporal para o grafico |
| `GET` | `/ws` | WebSocket — push de novas leituras em tempo real |

### Payload POST / PUT

```json
{
  "temperatura": 25.5,
  "umidade": 65.0,
  "pressao": 1013.2,
  "localizacao": "Lab"
}
```

Os campos `pressao` e `localizacao` sao opcionais.

---

## Fluxo MQTT

```
[ESP32] → publica JSON → [broker MQTT :1883] → [Backend Rust subscreve] → [SQLite]
                                                            |
                                                     [WebSocket push]
                                                            |
                                                    [Dashboard atualiza]
```

- **Broker:** configuravel via `CONFIG_MQTT_BROKER_URL` (firmware) e variavel `MQTT_BROKER` (simulador)
- **Topico:** `esp32/sensores`
- **Payload:**
  ```json
  {"temperatura": 25.5, "umidade": 65.0, "pressao": 1013.2, "localizacao": "Lab"}
  ```

---

## Criterios de Avaliacao

| Criterio | Peso | Status | Implementacao |
|---|---|---|---|
| Comunicacao Dispositivo → API | 20% | Atendido | ESP32 publica via MQTT; backend assina e persiste no banco. Simulador disponivel como alternativa sem hardware |
| API REST completa | 25% | Atendido | GET, POST, PUT, DELETE em `/api/leituras`, mais `/api/estatisticas` e `/api/grafico` |
| Banco de dados — schema e CRUD | 15% | Atendido | Tabela `leituras` identica ao schema sugerido; WAL mode ativo; migrations versionadas; 141 leituras no `dados.db` |
| Interface Web funcional | 20% | Atendido | `index.html` (dashboard), `historico.html` (paginacao + exclusao), `editar.html` (formulario PUT) |
| Grafico de variacao temporal | 10% | Atendido | Chart.js com series de temperatura e umidade, atualizado em tempo real via WebSocket |
| README com instrucoes | 5% | Atendido | Este documento |
| Organizacao e boas praticas | 5% | Atendido | Modulos separados (api, db, models, mqtt), migrations, CSS externo, `.gitignore` configurado |