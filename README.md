# CARAXES

### A local first development enviroment and agent runtime

#### Setup

```bash
cp .env.example .env
# then set OPENROUTER_API_KEY in .env
```

####  ideas
- we can also make the arch more simpler so that people can spin a pod / container and be satisfied with that but for without pod / container we cna add our boudaries and stuff for security.
- we can also take inspiration from orca for our application.
- need to add pre commit hook for  lint .

#### Logging / tracing

Normal runs are silent. Logs go to stderr, so stdout is only the streamed answer. Turn logging on when need it:

```bash
# simple level switch
CARAXES_LOG=debug cargo run -p caraxes_cli -- query "read README.md"  
# per-module
RUST_LOG=caraxes_internal::model::openai=trace cargo run -p caraxes_cli -- query "whatever" 
```


Release builds compile out `debug!`/`trace!` entirely via `tracing`'s `release_max_level_info` in `Cargo.toml`, so those calls cost nothing at runtime:

```bash
cargo build --release                          
# info+ only; env-filter still compiled in
cargo build --release --no-default-features    
# leanest: no env-filter/regex deps, fixed info level
```

Will be using the second form in CI/push builds for the smallest binary and fastest build. 

Note: `debug_assertions` is not a supported Cargo dependency selector, so the lean release build is selected with the `dev-logs` feature flag rather than automatically by profile.
