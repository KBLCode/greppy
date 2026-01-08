//! Local query expansion without LLM
//!
//! Builds synonym maps from indexed symbols for instant query expansion.
//! Falls back to LLM only for truly ambiguous natural language queries.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use tracing::{debug, info};

use crate::index::IndexSearcher;

/// Common code-related synonyms that don't need LLM
/// Comprehensive dictionary covering all major programming domains
const BUILTIN_SYNONYMS: &[(&str, &[&str])] = &[
    // ═══════════════════════════════════════════════════════════════════════════
    // AUTHENTICATION & SECURITY
    // ═══════════════════════════════════════════════════════════════════════════
    ("auth", &["authenticate", "authentication", "login", "logout", "session", "token", "credential", "password", "jwt", "oauth", "sso", "saml", "identity", "principal", "user", "account"]),
    ("login", &["auth", "authenticate", "signin", "sign_in", "logon", "credentials", "session"]),
    ("logout", &["signout", "sign_out", "logoff", "disconnect", "revoke"]),
    ("permission", &["role", "access", "authorize", "authorization", "rbac", "abac", "acl", "grant", "deny", "policy", "scope", "claim"]),
    ("security", &["auth", "encrypt", "decrypt", "hash", "salt", "secure", "vulnerability", "xss", "csrf", "injection", "sanitize", "escape"]),
    ("token", &["jwt", "bearer", "refresh", "access", "apikey", "secret", "credential"]),
    ("encrypt", &["decrypt", "cipher", "aes", "rsa", "crypto", "cryptography", "hash", "hmac"]),
    ("password", &["passwd", "secret", "credential", "hash", "bcrypt", "argon", "scrypt"]),
    ("session", &["cookie", "token", "stateful", "stateless", "expire", "ttl"]),
    ("oauth", &["oidc", "openid", "sso", "saml", "auth0", "keycloak", "identity"]),
    
    // ═══════════════════════════════════════════════════════════════════════════
    // ERRORS & EXCEPTIONS
    // ═══════════════════════════════════════════════════════════════════════════
    ("error", &["err", "exception", "failure", "fail", "panic", "crash", "bug", "fault", "issue", "problem", "invalid", "unexpected"]),
    ("handle", &["handler", "handling", "catch", "process", "manage", "deal", "cope"]),
    ("try", &["catch", "except", "result", "option", "maybe", "either", "unwrap"]),
    ("throw", &["raise", "panic", "bail", "abort", "reject", "fail"]),
    ("recover", &["retry", "fallback", "backup", "restore", "resume", "resilient", "graceful"]),
    ("debug", &["debugger", "breakpoint", "inspect", "trace", "step", "watch", "diagnose"]),
    ("stacktrace", &["backtrace", "callstack", "traceback", "stack", "frame"]),
    
    // ═══════════════════════════════════════════════════════════════════════════
    // CRUD OPERATIONS
    // ═══════════════════════════════════════════════════════════════════════════
    ("create", &["new", "add", "insert", "make", "build", "init", "initialize", "construct", "instantiate", "spawn", "generate", "produce"]),
    ("read", &["get", "fetch", "load", "find", "query", "select", "retrieve", "lookup", "search", "list", "show", "view"]),
    ("update", &["edit", "modify", "change", "set", "patch", "put", "mutate", "alter", "replace", "upsert", "merge"]),
    ("delete", &["remove", "destroy", "drop", "clear", "purge", "erase", "unlink", "dispose", "cleanup", "gc"]),
    ("save", &["store", "persist", "write", "commit", "flush", "sync"]),
    ("copy", &["clone", "duplicate", "replicate", "fork", "deep", "shallow"]),
    
    // ═══════════════════════════════════════════════════════════════════════════
    // DATA & STORAGE
    // ═══════════════════════════════════════════════════════════════════════════
    ("database", &["db", "sql", "postgres", "postgresql", "mysql", "sqlite", "mongo", "mongodb", "redis", "store", "storage", "repository", "datastore", "dynamo", "cassandra"]),
    ("cache", &["cached", "caching", "memoize", "lru", "ttl", "invalidate", "evict", "warm", "cold", "hit", "miss"]),
    ("config", &["configuration", "settings", "options", "preferences", "env", "environment", "params", "parameters", "dotenv", "yaml", "toml", "json"]),
    ("schema", &["model", "entity", "table", "collection", "structure", "definition", "type", "interface", "migration", "ddl"]),
    ("query", &["search", "find", "filter", "where", "select", "lookup", "fetch", "criteria", "predicate"]),
    ("index", &["indexing", "indexed", "reindex", "search", "lookup", "btree", "hash", "fulltext"]),
    ("transaction", &["tx", "txn", "commit", "rollback", "atomic", "acid", "isolation", "lock"]),
    ("migration", &["migrate", "schema", "alter", "ddl", "upgrade", "downgrade", "version"]),
    ("orm", &["activerecord", "sequelize", "prisma", "typeorm", "sqlalchemy", "hibernate", "entity"]),
    ("nosql", &["mongo", "mongodb", "dynamo", "dynamodb", "cassandra", "couchdb", "document", "keyvalue"]),
    ("queue", &["message", "broker", "rabbitmq", "kafka", "sqs", "pubsub", "amqp", "redis"]),
    
    // ═══════════════════════════════════════════════════════════════════════════
    // API & NETWORK
    // ═══════════════════════════════════════════════════════════════════════════
    ("api", &["endpoint", "route", "handler", "controller", "rest", "graphql", "rpc", "service", "resource"]),
    ("request", &["req", "http", "fetch", "call", "invoke", "send", "post", "get", "put", "patch", "delete"]),
    ("response", &["res", "reply", "result", "return", "output", "answer", "payload", "body"]),
    ("middleware", &["interceptor", "filter", "hook", "plugin", "pipe", "chain", "layer"]),
    ("websocket", &["ws", "wss", "socket", "realtime", "push", "stream", "sse", "eventsource", "bidirectional"]),
    ("client", &["consumer", "caller", "requester", "frontend", "sdk", "library"]),
    ("server", &["backend", "service", "daemon", "host", "listener", "worker"]),
    ("http", &["https", "request", "response", "header", "body", "status", "method", "url", "uri"]),
    ("rest", &["restful", "api", "crud", "resource", "endpoint", "json", "xml"]),
    ("graphql", &["query", "mutation", "subscription", "resolver", "schema", "apollo", "relay"]),
    ("grpc", &["protobuf", "proto", "rpc", "streaming", "unary", "bidirectional"]),
    ("cors", &["crossorigin", "origin", "preflight", "header", "access"]),
    ("proxy", &["reverse", "forward", "gateway", "load", "balancer", "nginx", "haproxy"]),
    ("url", &["uri", "path", "route", "endpoint", "link", "href", "slug"]),
    ("header", &["headers", "authorization", "content", "accept", "cookie", "origin"]),
    ("status", &["code", "http", "success", "error", "redirect", "client", "server"]),
    
    // ═══════════════════════════════════════════════════════════════════════════
    // ASYNC & CONCURRENCY
    // ═══════════════════════════════════════════════════════════════════════════
    ("async", &["await", "promise", "future", "concurrent", "parallel", "thread", "spawn", "task", "coroutine", "generator"]),
    ("sync", &["synchronous", "blocking", "sequential", "serial", "mutex", "lock", "semaphore"]),
    ("channel", &["queue", "buffer", "pipe", "stream", "mpsc", "broadcast", "sender", "receiver"]),
    ("pool", &["pooling", "connection", "worker", "thread", "executor"]),
    ("mutex", &["lock", "rwlock", "semaphore", "atomic", "sync", "critical", "section"]),
    ("thread", &["threading", "multithread", "worker", "spawn", "join", "pool"]),
    ("promise", &["future", "async", "await", "then", "resolve", "reject", "pending"]),
    ("callback", &["cb", "handler", "listener", "hook", "continuation"]),
    ("race", &["condition", "deadlock", "livelock", "starvation", "contention"]),
    ("timeout", &["deadline", "expire", "ttl", "cancel", "abort"]),
    
    // ═══════════════════════════════════════════════════════════════════════════
    // TESTING
    // ═══════════════════════════════════════════════════════════════════════════
    ("test", &["spec", "unittest", "unit", "integration", "e2e", "mock", "stub", "fixture", "assert", "expect", "should", "describe", "it"]),
    ("mock", &["stub", "fake", "spy", "double", "dummy", "jest", "sinon", "vitest"]),
    ("assert", &["expect", "should", "verify", "check", "ensure", "must"]),
    ("fixture", &["setup", "teardown", "before", "after", "seed", "factory"]),
    ("coverage", &["cover", "lcov", "istanbul", "nyc", "codecov", "branch", "line"]),
    ("snapshot", &["snap", "golden", "baseline", "regression"]),
    ("benchmark", &["bench", "perf", "performance", "profile", "measure", "timing"]),
    
    // ═══════════════════════════════════════════════════════════════════════════
    // COMMON PATTERNS & TRANSFORMATIONS
    // ═══════════════════════════════════════════════════════════════════════════
    ("parse", &["parser", "parsing", "deserialize", "decode", "unmarshal", "read", "extract", "tokenize", "lex"]),
    ("serialize", &["encode", "stringify", "marshal", "dump", "format", "write", "tostring", "tojson"]),
    ("validate", &["validation", "validator", "check", "verify", "sanitize", "assert", "ensure", "guard", "constraint"]),
    ("transform", &["convert", "map", "translate", "adapt", "morph", "mutate", "pipe", "compose"]),
    ("format", &["formatter", "pretty", "beautify", "lint", "style", "indent", "minify"]),
    ("filter", &["where", "predicate", "criteria", "condition", "match", "select"]),
    ("sort", &["order", "orderby", "asc", "desc", "compare", "rank", "priority"]),
    ("group", &["groupby", "aggregate", "bucket", "partition", "cluster"]),
    ("merge", &["combine", "join", "concat", "union", "intersect", "diff"]),
    ("split", &["divide", "chunk", "partition", "segment", "slice", "tokenize"]),
    ("dedupe", &["deduplicate", "unique", "distinct", "dedup"]),
    ("flatten", &["flat", "unwrap", "spread", "expand"]),
    ("reduce", &["fold", "aggregate", "accumulate", "collect", "sum"]),
    ("map", &["transform", "convert", "project", "select", "apply"]),
    
    // ═══════════════════════════════════════════════════════════════════════════
    // UI & FRONTEND
    // ═══════════════════════════════════════════════════════════════════════════
    ("component", &["widget", "element", "view", "control", "ui", "module", "block"]),
    ("render", &["display", "draw", "paint", "show", "present", "output", "mount", "hydrate"]),
    ("style", &["css", "scss", "sass", "less", "theme", "design", "layout", "appearance", "tailwind", "styled"]),
    ("state", &["store", "context", "redux", "atom", "signal", "reactive", "observable", "mobx", "zustand", "recoil"]),
    ("event", &["listener", "handler", "callback", "emit", "dispatch", "trigger", "on", "click", "change", "submit"]),
    ("hook", &["usestate", "useeffect", "usememo", "usecallback", "useref", "usecontext", "custom"]),
    ("props", &["properties", "attributes", "params", "args", "input"]),
    ("dom", &["document", "element", "node", "virtual", "vdom", "shadow"]),
    ("animation", &["animate", "transition", "motion", "keyframe", "tween", "spring"]),
    ("responsive", &["mobile", "tablet", "desktop", "breakpoint", "media", "adaptive"]),
    ("modal", &["dialog", "popup", "overlay", "drawer", "sheet", "toast", "notification"]),
    ("form", &["input", "field", "submit", "validate", "formik", "hookform", "controlled"]),
    ("router", &["route", "navigation", "navigate", "link", "history", "path", "param"]),
    ("ssr", &["server", "hydrate", "hydration", "isomorphic", "universal", "ssg", "isr"]),
    
    // ═══════════════════════════════════════════════════════════════════════════
    // FILE & IO
    // ═══════════════════════════════════════════════════════════════════════════
    ("file", &["fs", "path", "directory", "folder", "io", "stream", "buffer", "blob", "binary"]),
    ("write", &["save", "store", "output", "dump", "export", "persist", "flush"]),
    ("watch", &["watcher", "monitor", "observe", "listen", "notify", "fsevents", "inotify", "chokidar"]),
    ("upload", &["multipart", "formdata", "blob", "file", "stream", "chunk"]),
    ("download", &["fetch", "stream", "blob", "save", "export"]),
    ("path", &["filepath", "dirname", "basename", "extension", "resolve", "join", "relative", "absolute"]),
    ("stream", &["readable", "writable", "duplex", "transform", "pipe", "buffer", "chunk"]),
    ("compress", &["zip", "gzip", "deflate", "tar", "archive", "decompress", "extract"]),
    
    // ═══════════════════════════════════════════════════════════════════════════
    // LOGGING & MONITORING
    // ═══════════════════════════════════════════════════════════════════════════
    ("log", &["logger", "logging", "trace", "debug", "info", "warn", "error", "print", "console", "stdout"]),
    ("metric", &["metrics", "stats", "statistics", "counter", "gauge", "histogram", "measure", "prometheus", "datadog"]),
    ("trace", &["tracing", "span", "telemetry", "observability", "apm", "opentelemetry", "jaeger", "zipkin"]),
    ("alert", &["alarm", "notify", "notification", "pager", "oncall", "incident"]),
    ("dashboard", &["grafana", "kibana", "datadog", "newrelic", "monitor", "visualize"]),
    ("health", &["healthcheck", "liveness", "readiness", "probe", "ping", "status"]),
    
    // ═══════════════════════════════════════════════════════════════════════════
    // PROCESS & LIFECYCLE
    // ═══════════════════════════════════════════════════════════════════════════
    ("start", &["init", "begin", "launch", "boot", "startup", "run", "execute", "main", "entry"]),
    ("stop", &["end", "finish", "terminate", "shutdown", "close", "halt", "kill", "exit", "quit"]),
    ("restart", &["reload", "refresh", "reset", "reboot", "respawn"]),
    ("deploy", &["deployment", "release", "publish", "ship", "rollout", "promote"]),
    ("build", &["compile", "bundle", "package", "assemble", "make", "webpack", "vite", "esbuild"]),
    ("install", &["setup", "configure", "provision", "bootstrap"]),
    
    // ═══════════════════════════════════════════════════════════════════════════
    // TYPES & DATA STRUCTURES
    // ═══════════════════════════════════════════════════════════════════════════
    ("type", &["typedef", "interface", "struct", "class", "enum", "union", "alias", "generic"]),
    ("string", &["str", "text", "char", "varchar", "utf8", "unicode", "ascii"]),
    ("number", &["int", "integer", "float", "double", "decimal", "numeric", "bigint"]),
    ("boolean", &["bool", "flag", "true", "false", "truthy", "falsy"]),
    ("array", &["list", "vector", "slice", "collection", "sequence", "tuple"]),
    ("object", &["dict", "dictionary", "map", "hashmap", "record", "struct", "hash"]),
    ("null", &["nil", "none", "undefined", "void", "empty", "nothing"]),
    ("optional", &["option", "maybe", "nullable", "undefined"]),
    ("generic", &["template", "parameterized", "polymorphic", "type"]),
    ("enum", &["enumeration", "variant", "union", "discriminated", "tagged"]),
    ("iterator", &["iter", "iterable", "cursor", "generator", "yield", "next"]),
    ("tree", &["node", "leaf", "branch", "root", "parent", "child", "sibling"]),
    ("graph", &["node", "edge", "vertex", "directed", "undirected", "weighted"]),
    ("linked", &["list", "node", "next", "prev", "head", "tail"]),
    ("stack", &["push", "pop", "lifo", "top"]),
    ("heap", &["priority", "queue", "min", "max", "heapify"]),
    
    // ═══════════════════════════════════════════════════════════════════════════
    // DESIGN PATTERNS
    // ═══════════════════════════════════════════════════════════════════════════
    ("singleton", &["instance", "global", "shared", "static"]),
    ("factory", &["create", "builder", "construct", "make", "produce"]),
    ("observer", &["subscribe", "publish", "notify", "listener", "event", "pubsub"]),
    ("strategy", &["policy", "algorithm", "behavior", "interchangeable"]),
    ("decorator", &["wrapper", "enhance", "extend", "augment", "mixin"]),
    ("adapter", &["wrapper", "bridge", "convert", "translate", "facade"]),
    ("proxy", &["delegate", "surrogate", "placeholder", "lazy"]),
    ("repository", &["repo", "dao", "dataaccess", "store", "persistence"]),
    ("service", &["provider", "manager", "handler", "controller", "usecase"]),
    ("dto", &["dataobject", "transfer", "payload", "model", "entity"]),
    ("dependency", &["inject", "injection", "di", "ioc", "container", "provider"]),
    
    // ═══════════════════════════════════════════════════════════════════════════
    // VERSION CONTROL & CI/CD
    // ═══════════════════════════════════════════════════════════════════════════
    ("git", &["commit", "push", "pull", "merge", "rebase", "branch", "checkout", "clone", "fetch"]),
    ("commit", &["push", "stage", "add", "message", "hash", "sha"]),
    ("branch", &["main", "master", "develop", "feature", "release", "hotfix"]),
    ("merge", &["rebase", "squash", "conflict", "resolve", "pr", "pullrequest"]),
    ("ci", &["cd", "pipeline", "workflow", "action", "job", "step", "stage"]),
    ("docker", &["container", "image", "dockerfile", "compose", "kubernetes", "k8s", "pod"]),
    ("kubernetes", &["k8s", "pod", "deployment", "service", "ingress", "helm", "kubectl"]),
    
    // ═══════════════════════════════════════════════════════════════════════════
    // CLOUD & INFRASTRUCTURE
    // ═══════════════════════════════════════════════════════════════════════════
    ("cloud", &["aws", "azure", "gcp", "serverless", "lambda", "function", "paas", "iaas"]),
    ("aws", &["amazon", "s3", "ec2", "lambda", "dynamodb", "sqs", "sns", "cloudfront"]),
    ("lambda", &["serverless", "function", "faas", "edge", "worker"]),
    ("container", &["docker", "kubernetes", "k8s", "pod", "image", "registry"]),
    ("scale", &["scaling", "autoscale", "horizontal", "vertical", "replica", "shard"]),
    ("load", &["balancer", "loadbalancer", "nginx", "haproxy", "alb", "elb"]),
    ("cdn", &["cloudfront", "cloudflare", "edge", "cache", "static", "asset"]),
    ("dns", &["domain", "record", "cname", "arecord", "nameserver", "resolve"]),
    ("ssl", &["tls", "https", "certificate", "cert", "letsencrypt", "acme"]),
    ("vpc", &["network", "subnet", "firewall", "security", "group", "cidr"]),
    
    // ═══════════════════════════════════════════════════════════════════════════
    // ALGORITHMS & COMPLEXITY
    // ═══════════════════════════════════════════════════════════════════════════
    ("algorithm", &["algo", "logic", "procedure", "method", "technique"]),
    ("search", &["find", "lookup", "binary", "linear", "bfs", "dfs", "astar"]),
    ("sort", &["quick", "merge", "heap", "bubble", "insertion", "radix", "bucket"]),
    ("hash", &["hashing", "hashmap", "hashtable", "digest", "md5", "sha", "checksum"]),
    ("recursive", &["recursion", "recur", "base", "case", "tail", "memoize"]),
    ("dynamic", &["dp", "programming", "memoization", "tabulation", "subproblem"]),
    ("greedy", &["optimal", "local", "choice", "heuristic"]),
    ("complexity", &["bigO", "time", "space", "constant", "linear", "logarithmic", "quadratic"]),
    
    // ═══════════════════════════════════════════════════════════════════════════
    // DOCUMENTATION & COMMENTS
    // ═══════════════════════════════════════════════════════════════════════════
    ("doc", &["docs", "documentation", "readme", "comment", "jsdoc", "tsdoc", "rustdoc", "javadoc"]),
    ("comment", &["note", "todo", "fixme", "hack", "xxx", "deprecated", "warning"]),
    ("readme", &["documentation", "guide", "tutorial", "example", "usage"]),
    ("changelog", &["history", "release", "notes", "version", "breaking"]),
    ("license", &["mit", "apache", "gpl", "bsd", "copyright", "open", "source"]),
    
    // ═══════════════════════════════════════════════════════════════════════════
    // LANGUAGE-SPECIFIC
    // ═══════════════════════════════════════════════════════════════════════════
    // Rust
    ("rust", &["cargo", "crate", "mod", "impl", "trait", "derive", "macro", "unsafe", "lifetime", "borrow"]),
    ("trait", &["interface", "protocol", "typeclass", "impl", "derive", "bound"]),
    ("lifetime", &["borrow", "reference", "ownership", "move", "copy", "clone"]),
    ("macro", &["derive", "proc", "declarative", "hygiene", "expand"]),
    
    // JavaScript/TypeScript
    ("javascript", &["js", "ecmascript", "es6", "es2015", "node", "deno", "bun"]),
    ("typescript", &["ts", "typed", "interface", "type", "generic", "infer"]),
    ("node", &["nodejs", "npm", "yarn", "pnpm", "require", "module", "commonjs", "esm"]),
    ("react", &["jsx", "tsx", "component", "hook", "state", "props", "context", "redux"]),
    ("vue", &["vuejs", "composition", "options", "reactive", "ref", "computed", "pinia"]),
    ("angular", &["ng", "component", "service", "module", "directive", "pipe", "rxjs"]),
    ("svelte", &["sveltekit", "reactive", "store", "action", "transition"]),
    ("next", &["nextjs", "ssr", "ssg", "isr", "app", "router", "middleware"]),
    
    // Python
    ("python", &["py", "pip", "venv", "conda", "django", "flask", "fastapi"]),
    ("django", &["model", "view", "template", "orm", "admin", "middleware"]),
    ("flask", &["route", "blueprint", "jinja", "werkzeug"]),
    ("fastapi", &["pydantic", "async", "openapi", "swagger", "uvicorn"]),
    
    // Go
    ("golang", &["go", "goroutine", "channel", "defer", "interface", "struct", "package"]),
    ("goroutine", &["concurrent", "channel", "select", "waitgroup", "mutex"]),
    
    // Java/Kotlin
    ("java", &["jvm", "spring", "maven", "gradle", "hibernate", "jpa"]),
    ("spring", &["boot", "mvc", "security", "data", "cloud", "bean", "autowired"]),
    ("kotlin", &["coroutine", "suspend", "flow", "sealed", "data", "companion"]),
    
    // ═══════════════════════════════════════════════════════════════════════════
    // COMMON ABBREVIATIONS
    // ═══════════════════════════════════════════════════════════════════════════
    ("id", &["identifier", "uuid", "guid", "key", "pk", "primary"]),
    ("uuid", &["guid", "id", "identifier", "unique", "random"]),
    ("url", &["uri", "link", "href", "path", "endpoint"]),
    ("json", &["object", "parse", "stringify", "serialize", "deserialize"]),
    ("xml", &["parse", "dom", "sax", "xpath", "xslt"]),
    ("csv", &["parse", "delimiter", "column", "row", "spreadsheet"]),
    ("regex", &["regexp", "pattern", "match", "replace", "capture", "group"]),
    ("env", &["environment", "variable", "config", "dotenv", "secret"]),
    ("cli", &["command", "terminal", "shell", "arg", "flag", "option"]),
    ("gui", &["ui", "interface", "window", "dialog", "widget"]),
    ("sdk", &["library", "client", "api", "wrapper", "package"]),
    ("pkg", &["package", "module", "library", "dependency", "crate"]),
    ("src", &["source", "code", "lib", "main", "app"]),
    ("tmp", &["temp", "temporary", "cache", "scratch"]),
    ("util", &["utility", "helper", "common", "shared", "lib"]),
    ("impl", &["implementation", "implement", "concrete", "realize"]),
    ("spec", &["specification", "test", "describe", "it", "should"]),
    ("init", &["initialize", "setup", "bootstrap", "start", "create"]),
    ("ctx", &["context", "state", "scope", "environment"]),
    ("req", &["request", "http", "input", "params"]),
    ("res", &["response", "result", "output", "reply"]),
    ("cb", &["callback", "handler", "listener", "function"]),
    ("fn", &["function", "func", "method", "procedure", "lambda"]),
    ("arg", &["argument", "param", "parameter", "input"]),
    ("val", &["value", "data", "result", "output"]),
    ("var", &["variable", "let", "const", "mut", "mutable"]),
    ("ref", &["reference", "pointer", "borrow", "alias"]),
    ("ptr", &["pointer", "reference", "address", "memory"]),
    ("buf", &["buffer", "array", "bytes", "data"]),
    ("len", &["length", "size", "count", "capacity"]),
    ("max", &["maximum", "limit", "upper", "bound", "cap"]),
    ("min", &["minimum", "lower", "bound", "floor"]),
    ("avg", &["average", "mean", "median", "aggregate"]),
    ("cnt", &["count", "total", "sum", "number"]),
    ("idx", &["index", "position", "offset", "cursor"]),
    ("prev", &["previous", "before", "prior", "last"]),
    ("curr", &["current", "now", "present", "active"]),
    ("temp", &["temporary", "tmp", "scratch", "interim"]),
];

/// Intent patterns for local detection
const INTENT_PATTERNS: &[(&str, &[&str], &str)] = &[
    // (intent, trigger words, expansion suffix)
    ("find_definition", &["find", "where", "locate", "show", "get"], "definition declaration impl"),
    ("find_usage", &["usage", "used", "uses", "call", "calls", "reference"], "usage reference call invoke"),
    ("understand_flow", &["how", "flow", "work", "process", "explain"], "flow process logic implementation"),
    ("find_error", &["error", "bug", "fix", "issue", "problem", "fail"], "error exception handle catch"),
];

/// Local query expander using indexed symbols and builtin synonyms
pub struct LocalExpander {
    /// Synonyms from builtin + extracted from index
    synonyms: HashMap<String, HashSet<String>>,
    /// All known symbols from the index
    known_symbols: HashSet<String>,
}

impl LocalExpander {
    /// Create a new local expander with builtin synonyms
    pub fn new() -> Self {
        let mut synonyms: HashMap<String, HashSet<String>> = HashMap::new();
        
        // Load builtin synonyms
        for (key, values) in BUILTIN_SYNONYMS {
            let set = synonyms.entry(key.to_string()).or_default();
            for v in *values {
                set.insert(v.to_string());
            }
            // Also add reverse mappings
            for v in *values {
                let reverse_set = synonyms.entry(v.to_string()).or_default();
                reverse_set.insert(key.to_string());
            }
        }
        
        Self {
            synonyms,
            known_symbols: HashSet::new(),
        }
    }

    /// Load symbols from an indexed project to enhance expansion
    pub fn load_from_index(&mut self, project_path: &Path) -> Result<(), crate::error::GreppyError> {
        let searcher = IndexSearcher::open(project_path)?;
        
        // Search for common terms to extract symbols
        let common_queries = ["", "a", "e", "i", "o", "u", "s", "t", "n", "r"];
        
        for query in common_queries {
            if let Ok(results) = searcher.search(query, 100) {
                for result in results {
                    if let Some(ref name) = result.symbol_name {
                        // Add symbol name
                        self.known_symbols.insert(name.to_lowercase());
                        
                        // Extract words from camelCase/snake_case
                        let words = split_identifier(name);
                        for word in &words {
                            if word.len() >= 3 {
                                self.known_symbols.insert(word.clone());
                            }
                        }
                        
                        // Build synonym relationships between words in same symbol
                        if words.len() > 1 {
                            for word in &words {
                                let set = self.synonyms.entry(word.clone()).or_default();
                                for other in &words {
                                    if word != other && other.len() >= 3 {
                                        set.insert(other.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        info!("Loaded {} symbols, {} synonym groups from index", 
              self.known_symbols.len(), self.synonyms.len());
        
        Ok(())
    }

    /// Check if we can expand locally without LLM
    pub fn can_expand_locally(&self, query: &str) -> bool {
        let words: Vec<&str> = query.split_whitespace().collect();
        
        // If query contains known symbols, we can expand locally
        for word in &words {
            let lower = word.to_lowercase();
            if self.known_symbols.contains(&lower) {
                return true;
            }
            if self.synonyms.contains_key(&lower) {
                return true;
            }
        }
        
        // Check for intent patterns
        let lower_query = query.to_lowercase();
        for (_, triggers, _) in INTENT_PATTERNS {
            for trigger in *triggers {
                if lower_query.contains(trigger) {
                    return true;
                }
            }
        }
        
        false
    }

    /// Expand query locally without LLM
    pub fn expand(&self, query: &str) -> LocalExpansion {
        let words: Vec<String> = query
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();
        
        let mut expanded_terms: HashSet<String> = HashSet::new();
        let mut detected_intent = "general".to_string();
        
        // Add original words
        for word in &words {
            expanded_terms.insert(word.clone());
        }
        
        // Detect intent from patterns
        let lower_query = query.to_lowercase();
        for (intent, triggers, expansion) in INTENT_PATTERNS {
            for trigger in *triggers {
                if lower_query.contains(trigger) {
                    detected_intent = intent.to_string();
                    for term in expansion.split_whitespace() {
                        expanded_terms.insert(term.to_string());
                    }
                    break;
                }
            }
        }
        
        // Expand each word using synonyms
        for word in &words {
            if let Some(syns) = self.synonyms.get(word) {
                for syn in syns {
                    expanded_terms.insert(syn.clone());
                }
            }
            
            // Also check if word is part of a known symbol
            for symbol in &self.known_symbols {
                if symbol.contains(word) && symbol != word {
                    expanded_terms.insert(symbol.clone());
                }
            }
        }
        
        // Build expanded query string
        let expanded_query: Vec<String> = expanded_terms.into_iter().collect();
        
        debug!("Local expansion: '{}' -> '{}'", query, expanded_query.join(" "));
        
        LocalExpansion {
            intent: detected_intent,
            expanded_query: expanded_query.join(" "),
            used_llm: false,
        }
    }
}

impl Default for LocalExpander {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of local query expansion
#[derive(Debug, Clone)]
pub struct LocalExpansion {
    pub intent: String,
    pub expanded_query: String,
    pub used_llm: bool,
}

/// Split camelCase or snake_case identifier into words
fn split_identifier(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    
    for ch in name.chars() {
        if ch == '_' || ch == '-' {
            if !current.is_empty() {
                words.push(current.to_lowercase());
                current.clear();
            }
        } else if ch.is_uppercase() && !current.is_empty() {
            words.push(current.to_lowercase());
            current.clear();
            current.push(ch);
        } else {
            current.push(ch);
        }
    }
    
    if !current.is_empty() {
        words.push(current.to_lowercase());
    }
    
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_identifier() {
        assert_eq!(split_identifier("getUserById"), vec!["get", "user", "by", "id"]);
        assert_eq!(split_identifier("get_user_by_id"), vec!["get", "user", "by", "id"]);
        assert_eq!(split_identifier("HTTPRequest"), vec!["h", "t", "t", "p", "request"]);
        assert_eq!(split_identifier("parseJSON"), vec!["parse", "j", "s", "o", "n"]);
    }

    #[test]
    fn test_builtin_synonyms() {
        let expander = LocalExpander::new();
        
        // Auth should expand
        let result = expander.expand("auth");
        assert!(result.expanded_query.contains("login"));
        assert!(result.expanded_query.contains("token"));
        
        // Error should expand
        let result = expander.expand("error handling");
        assert!(result.expanded_query.contains("exception"));
        assert!(result.expanded_query.contains("catch"));
    }

    #[test]
    fn test_intent_detection() {
        let expander = LocalExpander::new();
        
        let result = expander.expand("how does auth work");
        assert_eq!(result.intent, "understand_flow");
        
        let result = expander.expand("find the user class");
        assert_eq!(result.intent, "find_definition");
    }

    #[test]
    fn test_can_expand_locally() {
        let expander = LocalExpander::new();
        
        assert!(expander.can_expand_locally("auth login"));
        assert!(expander.can_expand_locally("how does it work"));
        assert!(expander.can_expand_locally("error handling"));
    }
}
