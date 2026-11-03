# Repository agent instructions

## Required post-modification workflow

After finishing any modification in this repository:

1. Run the validation appropriate for the change and fix any failures.
2. Start the local Web debug app from the repository root with:

   ```sh
   python3 scripts/debug-web.py --open
   ```

   This command runs `trunk serve --open` together with the loopback navigation-log sink.
3. Keep that process running so the app remains open and `forma-debug.log` remains available for monitoring. Do not terminate it during the task; leave it running until the agent is stopped or the user explicitly asks to stop or restart it.
4. If an existing debug server is stale or occupies the configured ports, stop only that stale Forma debug process, then start the command above again.

