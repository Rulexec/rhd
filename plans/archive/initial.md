I want to develop tool in Rust, which will help me to automate some general tasks with help of AI tools.

This will be multi-crate workspace. All crates should be prefixed with `rhd_` and placed to `packages` folder (and then linked as workspace crates in the root Cargo.toml).

Main crate will be `rhd_app` where main binary will be located. I need crate `rhd_util` where we will store utils used in other crates. Crate `rhd_ai` will implement interacting with ai-platforms. For first — it should support only OpenAI compatible api.

rhd_app should have `daemon` argument which will start app which will create unix socket and wait for commands. Also rhd_app should have `run <name>` command, which will connect to this unix socket and tell daemon part to run scenario named `<name>`.

In the folder where `rhd daemon` was run there will be `scenarios` folder, in this folder -- different scenario folders, containing `scenario.yaml`. Like `scenarios/example/scenario.yaml`.

Scenario — is a simple chain of actions which daemon should execute sequentally. Action have types. For now there is only three types:

- runCommand
- aiChat
- output

`runCommand` have options: `cwd`, `cmd`, `args` (args is a list), which mean that command should be executed, then we need to remember exit code, stdout and stderr combined (just as string with newlines).

`aiChat` have options:

- `model` — which model we should use
- `systemPrompt` — yaml string
— `message` — yaml string

`output` have single option:

— `output`

Every step in scenario can have name. Then, in `aiChat` command in the `message` option we can use special placeholder. For example, runCommand name was `testsRun`. Then in `aiChat` step we can use `%testsRun.exitCode%`, `%testsRun.stdoutStderr`, which will be replaced with command execution exit code and stdout combined with stderr. `output` type can use placeholders too. `aiChat` step emits `message` placeholder, so then it can be used in `output` step like `%aiChatStep.message%` to form final output.

Then, when user does `rhn run my_scenario`, rhd daemon receives this command, checks contains of `scenarios/my_scenario/scenario.yaml`, then runs its steps, replacing placeholders, executing it. Then final step must be `output`, which will emit resulting text, which daemon should send to initial `rhn run` and `rhn run my_scenario` will print it and exit with zero exit code. It there is some errors — then it should exit with status code 1 and print some error details.