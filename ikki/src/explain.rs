use crate::docker_config::{BuildOptions, RunOptions};

impl BuildOptions {
    pub fn explain(&self) -> String {
        let mut s = String::new();

        if self.path.is_none() {
            s.push_str("docker pull ");
            s.push_str(self.pull.as_ref().unwrap());
            return s;
        }

        s.push_str("docker build ");

        // build-args
        for (name, value) in &self.build_args {
            let arg = format!("--build-arg {}={} ", name, value);
            s.push_str(&arg);
        }

        // tag
        let tag = format!("--tag {} ", &self.tag);
        s.push_str(&tag);

        // path
        s.push_str(&self.path.as_ref().unwrap().display().to_string());

        s
    }
}

impl RunOptions {
    pub fn explain(&self) -> String {
        let mut s = String::new();

        s.push_str("docker run");

        // name
        let name = format!(" --name {}", self.container_name);
        s.push_str(&name);

        // env
        for kv in &self.env {
            let env = format!(" --env {}", kv);
            s.push_str(&env);
        }

        // ports
        for port in &self.ports {
            let publish = format!(" --publish {}", port);
            s.push_str(&publish);
        }

        s.push(' ');
        s.push_str(&self.image_name);

        s
    }
}
