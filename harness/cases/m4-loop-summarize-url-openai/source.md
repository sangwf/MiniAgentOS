MiniAgentOS M4 uses an honest tool loop instead of hard-coded action branches.
The bootstrap runtime keeps session history in memory and lets the model call
bounded tools such as fetch_url before returning a final answer to the terminal.
