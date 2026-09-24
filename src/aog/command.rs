// Copyright (c) 2020-2021 Caleb Mitchell Smith (PixelCoda)
//
// MIT License
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
// THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use crate::aog;
use std::error::Error;

/// Console-only credential actions stay separate from the remote command API.
/// All shared commands use one parser and return their actual result.
pub fn run(cmd: String) -> Result<(), Box<dyn Error>> {
    let parts: Vec<_> = cmd.split_whitespace().collect();
    match parts.as_slice() {
        ["api", "token", "generate"] => {
            let token = aog::auth::set_api_token()?;
            println!("New Command API token generated: {}", token);
            println!("Store this token securely. It is required for remote command requests.");
        }
        ["api", "token", "remove"] => {
            aog::auth::remove_api_token()?;
            println!("Command API token removed. Only local commands are accepted until a new token is configured.");
        }
        ["api", "token", "status"] => {
            let config = crate::Config::load(0)?;
            if config
                .command_api_token
                .as_ref()
                .is_some_and(|token| !token.is_empty())
            {
                println!(
                    "Command API token is configured; command requests require authentication."
                );
            } else if config.command_api_token.is_some() {
                println!(
                    "The configured Command API token is empty; command requests are disabled."
                );
            } else {
                println!(
                    "No Command API token configured; only local command requests are accepted."
                );
            }
        }
        _ => {
            let show_local_help = parts.as_slice() == ["help"];
            println!("{}", aog::command_exec::execute_with_output(cmd)?);
            if show_local_help {
                println!("  api token generate | remove | status - Manage command authentication locally");
            }
        }
    }
    Ok(())
}
