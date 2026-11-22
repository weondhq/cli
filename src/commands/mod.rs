pub mod connect;
pub mod deploy;
pub mod disconnect;
pub mod logs;
pub mod projects;
pub mod whoami;

#[derive(Debug, Clone)]
pub struct Context {
    pub api_base: String,
    pub verbose: bool,
}
