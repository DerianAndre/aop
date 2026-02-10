# Especificación Técnica: Infraestructura de Comunicación y Memoria Semántica

**Documento de Ingeniería**: AOP-DET-001  
**Referencia de Sistema**: system.md (AOP_Master_Engineering_Prompt_v2.md)  
**Arquitecto**: Derian Castillo (Lead Systems Architect)  
**Nivel de Confidencialidad**: Operativo Crítico  
**Última Actualización**: Febrero 2026

---

## 0. Dependencias y Versiones

Todas las versiones están verificadas como estables a febrero 2026.

### Rust (Cargo.toml) — Dependencias específicas de este documento

| Crate | Versión | Propósito |
|---|---|---|
| tree-sitter | 0.26.3 | Parser AST incremental para fragmentación de código |
| tree-sitter-typescript | 0.23.2 | Gramática TypeScript/TSX para tree-sitter |
| tree-sitter-rust | 0.23.2 | Gramática Rust para tree-sitter |
| tree-sitter-javascript | 0.23.1 | Gramática JavaScript para tree-sitter |
| notify | 8.2.0 | Watcher de filesystem cross-platform (eventos de cambio) |
| ort | 2.0.0-rc.11 | ONNX Runtime bindings — inferencia local de embeddings |
| lancedb | 0.23 | Base de datos vectorial embebida, sin servidor |
| arrow | 57.2 | Formato columnar para intercambio con LanceDB |
| sha2 | 0.10.9 | Hash SHA-256 para detección de cambios |

> **Nota**: Las dependencias compartidas con el núcleo de AOP (tauri, sqlx, serde, tokio, uuid, chrono) están definidas en `system.md` y NO se duplican aquí.

### Node.js (package.json) — MCP Sidecar

| Paquete | Versión | Propósito |
|---|---|---|
| @modelcontextprotocol/sdk | ^1.26.0 | SDK oficial MCP (transporte stdio + JSON-RPC 2.0) |
| zod | ^3.25.0 | Validación de schemas (peer dependency del SDK) |
| typescript | ^5.7.0 | Compilador TypeScript |

---

## 1. Arquitectura del Universal MCP Bridge (Sidecar)

El puente MCP actúa como la capa de abstracción entre el núcleo soberano de AOP y los servidores MCP de los proyectos objetivos. Se implementa como un proceso hijo (Sidecar) para garantizar el aislamiento de fallos y la compatibilidad con el ecosistema Node.js.

### 1.1 Protocolo de Transporte y Serialización

- **Transporte**: stdio (entrada/salida estándar) utilizando pipes bidireccionales.
- **Formato de Mensaje**: JSON-RPC 2.0.
- **Concurrencia**: `Promise.allSettled` para procesar múltiples llamadas de herramientas simultáneas desde el pool de agentes.

**Formato de mensaje (request)**:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "read_file",
    "arguments": {
      "path": "src/components/App.tsx"
    }
  }
}
```

**Formato de mensaje (response)**:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "// contenido del archivo..."
      }
    ]
  }
}
```

**Formato de mensaje (error)**:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32001,
    "message": "SECURITY_VIOLATION: path escapes project root",
    "data": {
      "requested_path": "/etc/passwd",
      "project_root": "/home/user/my-project"
    }
  }
}
```

### 1.2 Capa de Seguridad Zero-Trust

El puente no confía en las instrucciones del agente. Se implementan los siguientes validadores:

**Scope Guardian** (Path Sanitizer):
- Toda ruta recibida se normaliza con `path.resolve()`.
- Si el resultado queda fuera del directorio raíz del proyecto, la operación se aborta con error `SECURITY_VIOLATION`.
- **Protección contra symlinks**: Antes de resolver, se ejecuta `fs.realpathSync()` para detectar symlinks que apunten fuera del proyecto. Si `realpath !== resolvedPath`, se bloquea la operación.
- Rutas con `..`, `~`, o caracteres nulos se rechazan antes de normalizar.

**Tool Sandbox**:
- Solo se exponen herramientas declaradas explícitamente en `aop_config.json` del proyecto.
- Schema del archivo:

```json
{
  "$schema": "https://aop.dev/schemas/aop_config.v1.json",
  "project_root": "./",
  "mcp_servers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@anthropic/mcp-server-filesystem", "./src"],
      "allowed_tools": ["read_file", "list_directory", "search_files"],
      "denied_tools": ["write_file", "move_file", "delete_file"]
    }
  },
  "security": {
    "max_calls_per_minute": 120,
    "max_concurrent_calls": 10,
    "write_enabled": false,
    "allowed_extensions": [".ts", ".tsx", ".js", ".jsx", ".css", ".json", ".md"]
  }
}
```

**Rate Limiter**:
- Máximo de llamadas MCP por minuto configurable (default: 120).
- Máximo de llamadas concurrentes configurable (default: 10).
- Si se excede el límite, las llamadas se encolan con backpressure. Si la cola supera 50 items, se devuelve error `RATE_LIMIT_EXCEEDED`.

**Circuit Breaker**:
- Si un servidor MCP falla 5 veces consecutivas, se abre el circuito por 30 segundos.
- Durante el circuito abierto, todas las llamadas a ese servidor devuelven error `SERVER_UNAVAILABLE` sin intentar conexión.
- Después de 30s, se permite 1 llamada de prueba (half-open). Si tiene éxito, se cierra el circuito.

**Inmutable Read**:
- Por defecto, todas las operaciones en Fases 1-4 son de solo lectura.
- Los permisos de escritura se activan únicamente tras la validación de Shadow Testing.
- El campo `security.write_enabled` en `aop_config.json` controla esto explícitamente.

### 1.3 Ciclo de Vida del Sidecar

```
┌─────────┐    ┌───────────┐    ┌───────────┐    ┌──────────┐
│  INIT   │───▶│ HANDSHAKE │───▶│ EXECUTION │───▶│ SHUTDOWN │
└─────────┘    └───────────┘    └───────────┘    └──────────┘
     │              │                 │                │
     ▼              ▼                 ▼                ▼
  Tauri spawn   mcp_ready +      JSON-RPC 2.0     SIGTERM →
  sidecar       tool list        request/response  3s timeout →
  process       (3s timeout)     (with heartbeat)  SIGKILL
```

**Fases detalladas**:

1. **Init**: Tauri invoca el binario sidecar al arrancar la aplicación via `Command::new().stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())`.

2. **Handshake**: El sidecar envía un evento `mcp_ready` con la lista de herramientas disponibles.
   - **Timeout**: Si no se recibe `mcp_ready` en 3 segundos, se reintenta el spawn (máximo 3 intentos).
   - **Fallback**: Si después de 3 intentos no hay handshake, se emite evento `mcp_failed` al frontend con el error.

3. **Execution**: El núcleo de Rust despacha comandos JSON-RPC via stdin/stdout del proceso hijo.

4. **Heartbeat**: Cada 15 segundos, el núcleo envía un ping al sidecar. Si no hay respuesta en 5 segundos, se considera el sidecar muerto y se ejecuta el flujo de recuperación:
   - Log del estado al momento del fallo.
   - Kill del proceso zombie (SIGKILL).
   - Re-spawn automático (máximo 3 intentos por sesión).
   - Si el re-spawn falla, se notifica al usuario via el frontend.

5. **Shutdown**: Al cerrar AOP:
   - Se envía señal SIGTERM al sidecar.
   - Se espera 3 segundos para cierre limpio.
   - Si no responde, SIGKILL para evitar procesos huérfanos.
   - Se limpian los pipes de stdin/stdout/stderr.

---

## 2. Motor de Indexación Vectorial (Semantic Engine)

El motor vectorial proporciona la "Memoria a Largo Plazo" necesaria para que los agentes comprendan la arquitectura del código sin necesidad de leer todos los archivos en cada turno.

### 2.1 Estrategia de Fragmentación AST-Aware

A diferencia del chunking basado en caracteres, AOP descompone el código basándose en su estructura lógica utilizando `tree-sitter` (v0.26.3).

**Niveles de Granularidad**:

| Nivel | Nodos AST capturados | Ejemplo |
|---|---|---|
| **Funcional** | `function_declaration`, `method_definition`, `arrow_function`, `constructor` | `async function fetchUser(id: string) { ... }` |
| **Estructural** | `class_declaration`, `interface_declaration`, `type_alias_declaration`, `enum_declaration` | `interface UserRepository { ... }` |
| **Dependencia** | `import_statement`, `export_statement`, `export_default` | `import { UserService } from './services'` |

**Reglas de fragmentación**:
- Fragmento máximo: 500 tokens (estimado). Si un nodo AST excede este límite, se subdivide por nodos hijos directos.
- Fragmento mínimo: 50 tokens. Nodos menores se agrupan con su nodo padre.
- Cada fragmento incluye 2 líneas de contexto (antes y después) para mantener coherencia.
- Los comentarios JSDoc/TSDoc se adjuntan al nodo que documentan, nunca se separan.

**Gramáticas soportadas en v1**:
- TypeScript/TSX (`tree-sitter-typescript` 0.23.2)
- JavaScript/JSX (`tree-sitter-javascript` 0.23.1)
- Rust (`tree-sitter-rust` 0.23.2)
- JSON (`tree-sitter-json` 0.24.8)
- CSS (`tree-sitter-css` 0.23.2)
- Markdown (`tree-sitter-markdown` 0.4.1)

### 2.2 Pipeline de Ingestión y Embeddings

**Modelos soportados**:

| Modelo | Dimensiones | Tipo | Max Tokens | Uso |
|---|---|---|---|---|
| BGE-M3 (BAAI/bge-m3) | 1024 | Local via ONNX (`ort` crate) | 8192 | Default. Sin internet requerido. |
| text-embedding-3-small (OpenAI) | 1536 | Cloud API | 8191 | Fallback cuando ONNX falla o para mayor precisión. |

**Estrategia de fallback**:
1. Se intenta generar el embedding con BGE-M3 local.
2. Si ONNX falla (modelo corrupto, sin memoria, etc.), se cae al modelo cloud.
3. Si el cloud no está disponible (sin internet), el fragmento se encola en un buffer persistente (`pending_embeddings` table en SQLite).
4. Al detectar conectividad, se procesan los embeddings pendientes en batch.
5. **Batch size**: 32 fragmentos por llamada (local) / 16 fragmentos por llamada (cloud, para respetar rate limits de OpenAI).

**Frecuencia de actualización**:
- Basada en eventos del filesystem usando `notify` crate (v8.2.0).
- Solo se re-indexan archivos cuyo hash SHA-256 haya cambiado desde la última indexación.
- Debounce de 500ms para evitar re-indexaciones durante saves rápidos (ej: auto-save del IDE).
- Los archivos en `.gitignore`, `node_modules`, `target/`, `dist/`, y `build/` se excluyen automáticamente.

**Fórmula de Relevancia (Retrieval)**:

La puntuación de relevancia `S(c, q)` para un fragmento `c` dada una consulta `q` se define como:

```
S(c, q) = α · cosine_sim(embed(c), embed(q)) + (1 - α) · decay(c)
```

Donde:
- `α = 0.85` (prioridad semántica sobre recencia, configurable)
- `cosine_sim()` = similitud coseno entre los vectores de embedding del fragmento y la consulta
- `decay(c) = exp(-λ · days_since_modified(c))` donde `λ = 0.05` (factor de decaimiento temporal)
- `days_since_modified(c)` = días desde la última modificación del archivo fuente

> **Efecto práctico**: Un archivo modificado hace 1 día tiene decay ≈ 0.95. Un archivo sin tocar en 30 días tiene decay ≈ 0.22. Esto prioriza código activo sobre código legacy sin eliminar contexto histórico.

### 2.3 Esquema de Persistencia (LanceDB v0.23)

**Ubicación**: `/artifacts/{appId}/public/data/vector_store/`

| Campo | Tipo | Función |
|---|---|---|
| `id` | `string` (UUID v4) | Identificador único del fragmento. Primary key. |
| `vector` | `float32[N]` | Representación latente. N=1024 (BGE-M3) o N=1536 (text-embedding-3-small). |
| `content` | `string` | Código fuente original (truncado a 2000 chars si excede). |
| `language` | `string` | Lenguaje de programación (`typescript`, `rust`, `javascript`, etc). |
| `embedding_model` | `string` | Modelo usado para generar el vector (`bge-m3` o `text-embedding-3-small`). |
| `metadata.path` | `string` | Ruta relativa al archivo desde la raíz del proyecto. |
| `metadata.range` | `struct { start_line: u32, end_line: u32 }` | Líneas de inicio y fin del fragmento. |
| `metadata.node_type` | `string` | Categoría del nodo AST (ej. `method_definition`, `class_declaration`). |
| `metadata.hash` | `string` | Hash SHA-256 del archivo al momento de indexación. |
| `metadata.parent_node` | `string \| null` | Nombre del nodo padre (ej. nombre de la clase que contiene el método). |
| `indexed_at` | `timestamp` | Fecha/hora de indexación (UTC). |
| `file_modified_at` | `timestamp` | Fecha/hora de última modificación del archivo fuente. Usado para `decay()`. |

**Manejo de dimensiones duales**:

El esquema soporta vectores de diferente dimensión según el modelo de embedding. Cuando se cambia de modelo, los vectores existentes del modelo anterior se marcan como `stale` y se re-procesan en background. No se mezclan vectores de diferentes modelos en una misma búsqueda — el campo `embedding_model` filtra antes de calcular similitud.

**Índice ANN**:
- Tipo: IVF-PQ (Inverted File Index con Product Quantization).
- `nprobe`: 20 (particiones a buscar en query time).
- Se reconstruye el índice automáticamente cuando el número de fragmentos crece más de 20% desde la última construcción.

---

## 3. Interfaz de Integración (Rust ↔ Sidecar ↔ Vector)

### 3.1 Tipos de Datos (Rust Structs)

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextChunk {
    pub id: String,
    pub content: String,
    pub language: String,
    pub score: f32,
    pub metadata: ChunkMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub node_type: String,
    pub parent_node: Option<String>,
    pub hash: String,
    pub file_modified_at: String, // ISO 8601
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolRequest {
    pub server_id: String,
    pub tool: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResponse {
    pub content: Vec<ContentBlock>,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatus {
    pub total_files: u32,
    pub indexed_files: u32,
    pub pending_files: u32,
    pub stale_files: u32,
    pub last_indexed_at: Option<String>,
    pub index_size_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SidecarHealth {
    pub alive: bool,
    pub uptime_seconds: u64,
    pub available_servers: Vec<String>,
    pub circuit_breaker_status: std::collections::HashMap<String, String>, // server_id -> "closed" | "open" | "half-open"
}
```

### 3.2 Comandos de Tauri (API Interna)

```rust
// === Vector Engine ===

#[tauri::command]
async fn query_context(
    query: String,
    top_k: Option<u32>,       // default: 5
    language: Option<String>,  // filtro opcional por lenguaje
) -> Result<Vec<ContextChunk>, String>;

#[tauri::command]
async fn reindex_file(path: String) -> Result<(), String>;

#[tauri::command]
async fn reindex_project() -> Result<IndexStatus, String>;

#[tauri::command]
async fn get_index_status() -> Result<IndexStatus, String>;

// === MCP Bridge ===

#[tauri::command]
async fn call_mcp_tool(request: McpToolRequest) -> Result<McpToolResponse, String>;

#[tauri::command]
async fn list_mcp_servers() -> Result<Vec<String>, String>;

#[tauri::command]
async fn get_sidecar_health() -> Result<SidecarHealth, String>;

#[tauri::command]
async fn restart_sidecar() -> Result<(), String>;
```

### 3.3 Lógica de Recuperación de Agente

Flujo completo cuando un agente (Tier 2/3) necesita contexto:

```
Agente genera consulta semántica
        │
        ▼
Vector Engine: buscar top_k=5 fragmentos más relevantes
        │
        ▼
MCP Bridge: verificar si los archivos fuente han cambiado
  (comparar metadata.hash vs SHA-256 actual del archivo en disco)
        │
        ├── Sin cambios → Usar fragmentos del índice directamente
        │
        └── Con cambios → target_read en tiempo real
                │
                ▼
            Re-indexar el archivo cambiado (async, no bloquea)
                │
                ▼
            Hidratar contexto con contenido fresco
                │
                ▼
            Enviar al LLM con los fragmentos actualizados
```

**Reglas de hidratación**:
- Si más de 3 de los 5 fragmentos están stale, se hace un re-index completo del directorio afectado.
- El contexto hidratado incluye siempre: el fragmento + 2 líneas antes/después + la declaración de imports del archivo.
- El tamaño total del contexto inyectado no debe superar 8000 tokens por turno de agente.

---

## 4. Manejo de Errores

### 4.1 Códigos de Error Personalizados

| Código | Nombre | Descripción | Acción |
|---|---|---|---|
| -32001 | `SECURITY_VIOLATION` | Path fuera del proyecto o symlink malicioso | Log + abort + notificar usuario |
| -32002 | `RATE_LIMIT_EXCEEDED` | Más de N llamadas/minuto al MCP | Encolar o rechazar |
| -32003 | `SERVER_UNAVAILABLE` | Circuit breaker abierto para ese servidor | Reintentar después de cooldown |
| -32004 | `TOOL_NOT_ALLOWED` | Herramienta no declarada en aop_config.json | Abort silencioso |
| -32005 | `SIDECAR_DEAD` | Sidecar no responde al heartbeat | Re-spawn automático |
| -32006 | `INDEX_STALE` | Más del 50% del índice está desactualizado | Re-index en background |
| -32007 | `EMBEDDING_FAILED` | Modelo local y cloud fallaron | Encolar en pending_embeddings |
| -32008 | `WRITE_DENIED` | Intento de escritura en modo read-only | Abort + log |

### 4.2 Estrategia de Reintentos

```
Intento 1 → espera 0ms (inmediato)
Intento 2 → espera 500ms
Intento 3 → espera 2000ms (backoff exponencial)
Intento 4+ → No reintentar. Emitir error al agente.
```

Los errores `SECURITY_VIOLATION`, `TOOL_NOT_ALLOWED` y `WRITE_DENIED` **nunca** se reintentan.

---

## 5. Roadmap de Implementación

### Fase 1: Cimientos de Comunicación

- [ ] Implementar el Sidecar en Node.js usando `@modelcontextprotocol/sdk` v1.26.0 con transporte stdio.
- [ ] Configurar el `command_handler` en Rust para gestionar stdin/stdout/stderr del Sidecar.
- [ ] Implementar el handshake inicial con timeout de 3s y 3 reintentos.
- [ ] Implementar el heartbeat (ping cada 15s, timeout 5s).
- [ ] Implementar el Scope Guardian con protección contra symlinks.
- [ ] Implementar el Rate Limiter (120 calls/min default).
- [ ] Crear el schema de `aop_config.json` y su parser/validador.

**Done when**:
- El sidecar arranca, hace handshake, y responde a llamadas `tools/call` desde Rust.
- Un path malicioso (ej: `../../etc/passwd`) es bloqueado y loggeado.
- El sidecar se recupera automáticamente después de un kill manual.
- El rate limiter bloquea la llamada #121 en un minuto.

### Fase 2: Inteligencia Semántica

- [ ] Integrar `tree-sitter` v0.26.3 en el worker de indexación de Rust con gramáticas TS/JS/Rust.
- [ ] Implementar la lógica de fragmentación AST-aware con los 3 niveles de granularidad.
- [ ] Configurar LanceDB v0.23 con el esquema completo (incluyendo `id`, `language`, `embedding_model`, `file_modified_at`).
- [ ] Implementar el servicio de embeddings dual (BGE-M3 local + text-embedding-3-small cloud).
- [ ] Implementar el fallback automático local → cloud → cola pendiente.
- [ ] Implementar el watcher de filesystem con `notify` v8.2.0 y debounce de 500ms.
- [ ] Implementar la fórmula de relevancia `S(c, q)` con decay temporal.

**Done when**:
- Un repositorio TypeScript de 200+ archivos se indexa en menos de 30 segundos.
- Una búsqueda semántica retorna 5 fragmentos relevantes en menos de 100ms.
- Al modificar un archivo, se re-indexa automáticamente sin intervención del usuario.
- Si se desconecta internet, los embeddings se encolan y se procesan al reconectar.

### Fase 3: Orquestación de Swarm

- [ ] Crear el "Context Provider" en React que visualice qué fragmentos de código están alimentando al agente en tiempo real.
- [ ] Implementar el Circuit Breaker para servidores MCP.
- [ ] Realizar pruebas de estrés con repositorios de >5,000 archivos para optimizar la latencia de búsqueda ANN.
- [ ] Implementar la lógica de re-hidratación de contexto (sección 3.3).
- [ ] Implementar métricas de observabilidad: latencia de embedding, hit rate del índice, frecuencia de re-index.

**Done when**:
- Un repositorio de 5,000+ archivos mantiene latencia de búsqueda < 200ms.
- El Circuit Breaker se abre después de 5 fallos consecutivos y se recupera después de 30s.
- El Context Provider muestra en tiempo real los fragmentos que el agente está usando.
- Las métricas de observabilidad están disponibles en el dashboard de AOP.

---

## 6. Decisiones de Arquitectura (ADRs)

**ADR-001: ¿Por qué Sidecar en Node.js y no Rust puro?**
El SDK oficial de MCP (`@modelcontextprotocol/sdk`) es TypeScript-first. Implementar el protocolo MCP desde cero en Rust requeriría reimplementar JSON-RPC 2.0, el handshake, y todos los tipos. El costo de mantenimiento sería alto y la compatibilidad con servidores MCP existentes no estaría garantizada. El sidecar en Node.js es la opción pragmática.

**ADR-002: ¿Por qué LanceDB y no Qdrant/Milvus?**
LanceDB es embebido (sin servidor), se integra nativamente con Arrow (que ya usamos), y escala a millones de vectores en disco. Qdrant y Milvus requieren un servidor separado, lo cual contradice la filosofía de AOP de ser una aplicación desktop autónoma.

**ADR-003: ¿Por qué BGE-M3 como modelo local default?**
BGE-M3 soporta 100+ idiomas, procesa hasta 8192 tokens, y produce embeddings de 1024 dimensiones (más ligero que los 1536 de OpenAI). El modelo ONNX pesa ~543MB en int8, lo cual es aceptable para una aplicación desktop. Su rendimiento en benchmarks de code retrieval es competitivo con modelos cloud.

**ADR-004: ¿Por qué tree-sitter y no regex/líneas para fragmentar?**
El chunking por líneas o regex pierde la estructura semántica del código. Un método partido a la mitad es inútil para un agente. tree-sitter produce un AST real que permite cortar por fronteras lógicas (funciones, clases, interfaces). El costo adicional de parsing es ~6ms por archivo de 2000 líneas.
