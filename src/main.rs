use std::{
    env,
    io::{self, Write, pipe},
    process::{Command, Stdio},
};

fn main() {
    init_shell();

    loop {
        print!("egg> ");
        io::stdout().flush().unwrap();

        let commands = read_input();

        if commands.is_empty() || commands[0].is_empty() {
            continue;
        }

        run_pipeline(&commands);
    }
}

fn init_shell() {
    println!("🥚 Welcome to eggshell! Type 'eggxit' to escape.");
}

fn read_input() -> Vec<Vec<String>> {
    let mut input = String::new();

    if io::stdin().read_line(&mut input).is_err() {
        eprintln!("Shellshock! Couldn't read input.");
        return vec![];
    }

    input
        .trim()
        .split('|')
        .map(|s| s.trim().split_whitespace().map(str::to_string).collect())
        .collect()
}

fn run_pipeline(commands: &[Vec<String>]) {
    if commands.is_empty() {
        return;
    }

    let mut previous_stdout = None;
    let mut children = Vec::new();

    for (i, command) in commands.iter().enumerate() {
        let stdin = match previous_stdout {
            Some(out) => Stdio::from(out),
            None => Stdio::inherit(),
        };

        let stdout = if i == commands.len() - 1 {
            Stdio::inherit()
        } else {
            Stdio::piped()
        };

        match spawn_command_with_io(command, stdin, stdout) {
            Ok(mut child) => {
                let stdout_pipe = child.stdout.take();
                previous_stdout = stdout_pipe;
                children.push(child);
            }
            Err(e) => {
                eprintln!("🥴 Error in pipeline: {}", e);
                return;
            }
        }
    }

    for mut child in children {
        let _ = child.wait();
    }
}

fn spawn_command_with_io(
    command: &[String],
    stdin: Stdio,
    stdout: Stdio,
) -> Result<std::process::Child, String> {
    if command.is_empty() {
        return Err("Empty command".to_string());
    }

    let program = &command[0];
    let args = &command[1..];

    match program.as_str() {
        "exit" | "eggxit" => {
            println!("🥚 Cracking out. Goodbye!");
            std::process::exit(0);
        }

        "cd" => {
            let new_dir = args.get(0).map(String::as_str).unwrap_or("/");
            if let Err(e) = env::set_current_dir(new_dir) {
                return Err(format!("cd: {}: {}", new_dir, e));
            }
            return Ok(Command::new("true").spawn().unwrap());
        }

        "echo" => {
            let message = args.join(" ") + "\n";
            return pipe_and_forward(message, stdout);
        }

        _ => {}
    }

    Command::new(program)
        .args(args)
        .stdin(stdin)
        .stdout(stdout)
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Failed to run '{}': {}", program, e))
}

fn pipe_and_forward(message: String, stdout: Stdio) -> Result<std::process::Child, String> {
    let (reader, mut writer) = pipe().map_err(|e| format!("pipe error: {}", e))?;

    std::thread::spawn(move || {
        let _ = writer.write_all(message.as_bytes());
        // writer is dropped automatically here
    });

    Command::new("cat")
        .stdin(Stdio::from(reader))
        .stdout(stdout)
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Failed to forward pipe: {}", e))
}
