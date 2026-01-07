use std::env;

fn main() {
    println!("env_os            : {}", env::consts::OS);
    println!("env_arch          : {}", env::consts::ARCH);
    println!("logical_cpus      : {}", num_cpus::get());
    println!("physical_cpus     : {}", num_cpus::get_physical());
    println!("rustc_version     : {}", rustc_version_runtime::version());
    let meta = rustc_version_runtime::version_meta();
    println!("rustc_semver      : {}", meta.semver);
    println!(
        "rustc_commit_hash : {}",
        meta.commit_hash.unwrap_or_else(|| "unknown".into())
    );
    println!("rustc_channel     : {:?}", meta.channel);
}
