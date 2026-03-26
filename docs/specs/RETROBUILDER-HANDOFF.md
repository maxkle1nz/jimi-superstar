# RETROBUILDER HANDOFF — ANOMALY

> Last synced: 2026-03-25
> Phase: R4 → R5 transition (Operational Arc → Platform Arc)
> Brand: ⌜ ⛛ ANOMALY ⌟ (formerly JIMI SUPERSTAR)
> Audit: m1nd deep hunt (531 nodes, 1340 edges) — 2026-03-25

---

## 1. O Que É Este Documento

Este é o ponto de continuação canónico do RETROBUILDER.
Qualquer agente que entre neste projecto deve ler este ficheiro primeiro.
Ele contém o estado actual completo, a arquitectura em uso, e as direcções de construção específicas que faltam.

---

## 2. Identidade Do Sistema

| Campo | Valor |
|---|---|
| **Nome** | ⌜ ⛛ ANOMALY ⌟ |
| **Linguagem** | Rust (edition 2024) |
| **Framework HTTP** | Axum + Tokio |
| **Persistência** | SQLite WAL (`rusqlite`) |
| **DB default** | `./data/jimi.sqlite` |
| **Servidor** | `http://127.0.0.1:3000` |
| **Env vars** | `ANOMALY_DB_PATH`, `ANOMALY_ADDR` |

---

## 3. Arquitectura Actual (O Que Existe)

### 3.1 Kernel (`crates/jimi-kernel/`)

O kernel é o coração puro Rust do sistema. Módulos:

| Módulo | Função |
|---|---|
| `runtime` | `HouseRuntime` — orquestrador central com registos de sessões, mandalas, cápsulas, slots, memória, world state |
| `mandala` | `MandalaManifest` — identidade canónica do agente (self, execution_policy, memory_policy, capability_policy, active_snapshot, projection) |
| `durable` | `DurableStore` — migração SQLite e persistência de todas as entidades |
| `event` | `EventEnvelope` — sistema de eventos tipados (session_created, turn_started, capsule_installed, etc.) |
| `fieldvault` | `FieldVaultArtifact` — artefactos selados com níveis (open, protected, sealed, sacred) |
| `session` | Estado da sessão e lanes |
| `slot` | `PersonalitySlot` — binding state para cápsulas activas |
| `capability` | Descritores de capacidade declarada |
| `ids` | Tipos de ID canónicos (SessionId, TurnId, LaneId) |

### 3.2 Servidor (`apps/jimi-server/`)

Axum server monolítico com:

- **50+ rotas REST** cobrindo sessões, turnos, dispatches, mandalas, cápsulas, slots, artefactos, provedores, marketplace, aprovações, world state, autonomia, memória
- **WebSocket** em `/ws/events` para streaming de eventos em tempo real
- **Cockpit HTML** embutido como `const COCKPIT_HTML: &str` — interface completa single-page
- **Provider adapters** em `provider_adapter.rs` — suporte a Codex, Anthropic, Copilot (GitHub OAuth device flow), Google, Grok, OpenRouter  
- **Provider auth** em `provider_auth.rs` — detecção automática de credenciais
- **Transcript distiller** — worker background para distilação de transcritos
- **Autonomy loop** — ciclo autónomo de auto-propulsão da house

### 3.3 Cockpit UI

O cockpit vive dentro de `main.rs` como uma constante raw string HTML.
Usa o design system **ANOMALY**:

| Token | Valor | Uso |
|---|---|---|
| `--void` | `#030304` | Background profundo |
| `--matter` | `#ffffff` | Texto primário |
| `--anti-matter` | `#ff1100` | Acções críticas, alertas |
| `--resonance` | `#4a00e0` | Acentos, profundidade |
| `--stable-matter` | `#00ffcc` | Estados estáveis, sucesso |

Tipografia:
- **Cinzel** — headings conceptuais
- **Cormorant Garamond** — texto orgânico/literário
- **JetBrains Mono** — dados técnicos

---

## 4. O Que Está Feito (Done) ✅

- [x] Kernel Rust completo com runtime, mandalas, cápsulas, slots, fieldvault, memória
- [x] DurableStore SQLite com migração automática (24 tabelas)
- [x] Sistema de eventos tipados com broadcasting WebSocket
- [x] Provider lanes com fallback groups e routing configurável
- [x] Provider adapters para 6 provedores (Codex, Anthropic, Copilot, Google, Grok, OpenRouter)
- [x] Copilot GitHub OAuth device flow com persistência de token em `~/.config/anomaly/copilot_token.json`
- [x] 3-band memory architecture (hot, warm, archive) com relevância e promoção
- [x] Context packet assembly com world state slice
- [x] Summary checkpoints, memory bridges, re-synthesis triggers
- [x] Capsule trust classification e lifecycle management
- [x] Marketplace shell com import/export de cápsulas
- [x] Approval surface com grant/deny humano
- [x] World state substrate (workspace+process snapshots, git dirty, changed files)
- [x] Autonomy loop com ciclos, transições, e recomendações de próximo passo
- [x] Control plane com edição de memory policy em runtime
- [x] Macro status panorâmico (RETROBUILDER + L1GHT + subsistemas)
- [x] Cockpit HTML com design system ANOMALY (void, resonance, anti-matter, stable-matter)
- [x] Rebranding completo de JIMI SUPERSTAR → ANOMALY em todos os ficheiros
- [x] Chat WebSocket directo no Cockpit (`/ws/chat`) com streaming + timeout 30s
- [x] reqwest HTTP timeouts (30s) em todos os provider adapters

---

## 5. O Que Falta (Next) 🔨

### 5.1 Prioridade Imediata (R5 Phase)

- [ ] **Extrair o Cockpit HTML para ficheiro externo** — o `COCKPIT_HTML` dentro de `main.rs` é uma string gigante que dificulta manutenção. Deveria ser servido como ficheiro estático
- [ ] **Implementar `.fld` real** — o FieldVault actualmente faz stub de seal/unseal. Implementar o formato binário real de cápsulas exportáveis
- [ ] **Conectar Transcript Distiller a IA** — o distiller actualmente simula a síntese. Conectar ao provider lane real para gerar sumários semânticos
- [ ] **Slot unlock path** — `SlotBindingState::Locked` existe mas não há função de unlock. Adicionar `deactivate` / `unlock` ao slot manager

### 5.2 Prioridade Média

- [ ] **Slot Console multi-agente** — permitir activação simultânea de múltiplas cápsulas em slots diferentes
- [ ] **Mandala hot-swap** — trocar a mandala activa sem restart
- [ ] **Capsule sandbox** — isolamento de execução entre cápsulas guest vs. house
- [ ] **World State incremental** — diff-based ingest em vez de snapshot completo

### 5.3 Horizonte

- [ ] **Marketplace remoto** — discovery e pull de cápsulas de registries externos
- [ ] **Multi-house federation** — comunicação entre instâncias ANOMALY
- [ ] **ANSI terminal surface** — cockpit CLI nativo com cores ANSI (além do web cockpit)

---

## 6. Como Construir e Correr

```bash
cd ~/jimi-superstar
cargo build
cargo run --release -p jimi-server
# → ANOMALY server listening on http://127.0.0.1:3000
```

Para alterar a base de dados ou o endereço:
```bash
ANOMALY_DB_PATH=./custom.sqlite ANOMALY_ADDR=0.0.0.0:8080 cargo run --release -p jimi-server
```

---

## 7. Specs Constitucionais

Ler por esta ordem:

1. [`RETROBUILDER.md`](file:///Users/cosmophonix/jimi-superstar/docs/specs/RETROBUILDER.md) — doutrina de construção
2. [`CAPSULE-MEMORY-ARCHITECTURE.md`](file:///Users/cosmophonix/jimi-superstar/docs/specs/CAPSULE-MEMORY-ARCHITECTURE.md) — arquitectura de memória de 3 bandas
3. [`MANDALA-CAPSULE-MODEL.md`](file:///Users/cosmophonix/jimi-superstar/docs/specs/MANDALA-CAPSULE-MODEL.md) — modelo de identidade de agente
4. [`JIMI-CAPSULE-RUNTIME-MARKETPLACE-BLUEPRINT.md`](file:///Users/cosmophonix/jimi-superstar/docs/specs/JIMI-CAPSULE-RUNTIME-MARKETPLACE-BLUEPRINT.md) — blueprint do marketplace
5. [`JIMI-OVERVISION-WORLD-STATE.md`](file:///Users/cosmophonix/jimi-superstar/docs/specs/JIMI-OVERVISION-WORLD-STATE.md) — integração de world state
6. [`JIMI-CONTROL-PLANE-UI.md`](file:///Users/cosmophonix/jimi-superstar/docs/specs/JIMI-CONTROL-PLANE-UI.md) — superfície de controlo

---

## 8. Regras Para O Próximo Agente

1. **Não renomear crates** — `jimi-kernel` e `jimi-server` mantêm os nomes estruturais. O rebrand é na marca visível, não nos módulos Rust.
2. **Não refatorar o kernel sem ler as specs** — a arquitectura é intencional. Mandala, FieldVault, Capsule, Slot são conceitos constitucionais, não são patterns genéricos.
3. **O cockpit é um protótipo** — serve para validação operacional. O produto final ANOMALY terá uma interface premium separada.
4. **RETROBUILDER é o método** — construir de trás para frente, do centro para fora, grounding antes de scaffolding.
5. **Todas as mutações passam por eventos** — o padrão event-sourced é intencional. Nada muda sem `EventEnvelope`.
