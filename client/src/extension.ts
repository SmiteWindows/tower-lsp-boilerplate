/* --------------------------------------------------------------------------------------------
 * Copyright (c) Microsoft Corporation. All rights reserved.
 * Licensed under the MIT License. See License.txt in the project root for license information.
 * ------------------------------------------------------------------------------------------ */

/**
 * VSCode extension for the L programming language.
 *
 * This extension provides language support for the L programming language by implementing
 * a client for the Language Server Protocol (LSP). It connects to the L language server
 * and provides features such as syntax highlighting, code completion, goto definition,
 * and more.
 */

import {
  commands,
  ConfigurationChangeEvent,
  Disposable,
  ExtensionContext,
  OutputChannel,
  ProgressLocation,
  StatusBarAlignment,
  StatusBarItem,
  ThemeColor,
  window,
  workspace,
} from "vscode";

import {
  CloseAction,
  ErrorAction,
  Executable,
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  State,
} from "vscode-languageclient/node";

/**
 * The language client instance.
 * This is the main connection to the L language server.
 */
let client: LanguageClient;

/**
 * Status bar item to show the server status.
 */
let statusBarItem: StatusBarItem;

/**
 * Output channel for logging.
 */
let outputChannel: OutputChannel;

/**
 * Configuration change listener.
 */
let configChangeListener: Disposable;

/**
 * Debounce timer for configuration changes.
 */
let configChangeTimer: NodeJS.Timeout | undefined;

/**
 * Flag to track if the extension is being disposed.
 */
let isDisposing = false;

/**
 * Called when the extension is activated.
 *
 * This function is called when the extension is first loaded. It sets up the
 * language client and starts the connection to the L language server.
 *
 * @param _context The extension context
 */
export async function activate(context: ExtensionContext) {
  // Create an output channel for tracing LSP communication
  const traceOutputChannel = window.createOutputChannel("L Language Server trace");

  // Create a dedicated output channel for the extension
  outputChannel = window.createOutputChannel("L Language");
  context.subscriptions.push(outputChannel);

  // Create status bar item
  statusBarItem = window.createStatusBarItem(StatusBarAlignment.Right, 100);
  statusBarItem.text = "$(sync~spin) L Language Server";
  statusBarItem.tooltip = "L Language Server is starting...";
  statusBarItem.show();
  context.subscriptions.push(statusBarItem);

  // Log activation
  outputChannel.appendLine("[INFO] L Language extension is now active!");

  // Get configuration
  const config = workspace.getConfiguration("l-language-server");
  const maxProblems = config.get<number>("maxNumberOfProblems", 100);
  const customServerPath = config.get<string>("serverPath", "");

  // Try to locate the server executable
  let serverCommand: string | undefined;
  const path = await import("path");
  const fs = await import("fs");
  const os = await import("os");

  // Check if a custom server path is specified in settings
  if (customServerPath && customServerPath.trim() !== "") {
    serverCommand = customServerPath;
    outputChannel.appendLine(`[INFO] Using server from configuration: ${serverCommand}`);
  }
  // Check if a custom server path is specified in environment variables
  else if (process.env.SERVER_PATH) {
    serverCommand = process.env.SERVER_PATH;
    outputChannel.appendLine(`[INFO] Using server from SERVER_PATH: ${serverCommand}`);
  } else {
    // Attempt to locate the server in multiple possible locations
    const possiblePaths = [
      // Relative to the extension's dist directory (debug)
      path.join(
        __dirname,
        "..",
        "..",
        "target",
        "debug",
        "l-language-server" + (os.platform() === "win32" ? ".exe" : ""),
      ),
      // Relative to the extension's dist directory (release)
      path.join(
        __dirname,
        "..",
        "..",
        "target",
        "release",
        "l-language-server" + (os.platform() === "win32" ? ".exe" : ""),
      ),
      // Relative to the workspace root (when developing - debug)
      path.join(
        __dirname,
        "..",
        "..",
        "..",
        "target",
        "debug",
        "l-language-server" + (os.platform() === "win32" ? ".exe" : ""),
      ),
      // Relative to the workspace root (when developing - release)
      path.join(
        __dirname,
        "..",
        "..",
        "..",
        "target",
        "release",
        "l-language-server" + (os.platform() === "win32" ? ".exe" : ""),
      ),
      // Direct command (if installed globally)
      "l-language-server",
    ];

    outputChannel.appendLine(
      `[INFO] Searching for server in possible paths: ${possiblePaths.join(", ")}`,
    );

    // Find the first path that exists
    for (const possiblePath of possiblePaths) {
      outputChannel.appendLine(`[INFO] Checking path: ${possiblePath}`);
      try {
        if (fs.existsSync(possiblePath)) {
          serverCommand = possiblePath;
          outputChannel.appendLine(`[INFO] Found server at: ${serverCommand}`);
          break;
        }
      } catch (error) {
        outputChannel.appendLine(`[WARN] Error checking path ${possiblePath}: ${error}`);
      }
    }

    // If none of the paths worked, use the fallback
    if (!serverCommand) {
      serverCommand = "l-language-server";
      outputChannel.appendLine(
        `[INFO] No local server found, using fallback command: ${serverCommand}`,
      );
    }
  }

  // Ensure the server command is properly formatted for the platform
  serverCommand = path.normalize(serverCommand || "l-language-server");
  outputChannel.appendLine(`[INFO] Final server command: ${serverCommand}`);

  // Verify the server command exists
  try {
    if (!fs.existsSync(serverCommand) && !serverCommand.includes(path.sep)) {
      outputChannel.appendLine(
        `[WARN] Server command '${serverCommand}' not found in PATH. Make sure the server is installed or configured correctly.`,
      );
    }
  } catch (error) {
    outputChannel.appendLine(`[WARN] Could not verify server command: ${error}`);
  }

  // Configure the server executable
  const run: Executable = {
    command: serverCommand,
    options: {
      env: {
        ...process.env,
        // eslint-disable-next-line @typescript-eslint/naming-convention
        RUST_LOG: "debug", // Enable debug logging for the Rust server
      },
    },
  };

  // Server options for both debug and run modes
  const serverOptions: ServerOptions = {
    run,
    debug: run, // Use the same configuration for both modes
  };
  // Options to control the language client
  const clientOptions: LanguageClientOptions = {
    // Register the server for plain text documents with the 'l' language
    documentSelector: [{ scheme: "file", language: "l" }],

    // Synchronize configuration and file changes with the server
    synchronize: {
      // Notify the server about file changes to '.l files contained in the workspace
      fileEvents: workspace.createFileSystemWatcher("**/*.l"),
      // Notify the server about configuration changes
      configurationSection: "l-language-server",
    },

    // Use the trace output channel for logging
    traceOutputChannel,

    // Initialization options for the server
    initializationOptions: {
      maxProblems,
    },

    // Error handling and reconnection options
    errorHandler: {
      // Handle errors that occur during LSP communication
      error: (error: Error, message: any, count: number) => {
        outputChannel.appendLine(`[ERROR] LSP Error (${count}): ${error.message || error}`);
        outputChannel.appendLine(`[ERROR] Message: ${JSON.stringify(message)}`);
        outputChannel.appendLine(`[ERROR] Stack: ${error.stack || "No stack trace available"}`);

        // Show error to user after multiple consecutive errors
        if (count >= 5) {
          window.showErrorMessage(
            `L Language Server encountered ${count} errors. Check the "L Language" output channel for details.`,
          );
        }

        return { action: ErrorAction.Continue }; // Continue running the server despite errors
      },
      // Handle when the LSP connection is closed
      closed: () => {
        outputChannel.appendLine("[ERROR] LSP connection closed, attempting to restart...");
        return { action: CloseAction.Restart }; // Attempt to restart the connection
      },
    },
  };

  // Create the language client and start the connection
  client = new LanguageClient(
    "l-language-server", // Unique identifier for the server
    "L language server", // Human-readable name
    serverOptions,
    clientOptions,
  );

  // Add client to subscriptions so it's properly disposed
  context.subscriptions.push(client);

  // Set up status bar updates based on client state
  const stateChangeListener = client.onDidChangeState((state) => {
    updateStatusBar(state.newState);
  });
  context.subscriptions.push(stateChangeListener);

  // Set up configuration change listener
  configChangeListener = workspace.onDidChangeConfiguration(handleConfigurationChange);
  context.subscriptions.push(configChangeListener);

  // Register commands
  context.subscriptions.push(
    commands.registerCommand("l-language.restartServer", async () => {
      await restartServer();
    }),
  );

  // Start the language client
  try {
    outputChannel.appendLine("[INFO] Starting language client...");

    // Show progress notification
    await window.withProgress(
      {
        location: ProgressLocation.Notification,
        title: "Starting L Language Server",
        cancellable: false,
      },
      async () => {
        // Start the client and wait for it to be ready
        await client.start();
        outputChannel.appendLine("[INFO] L Language Server started successfully");
      },
    );
  } catch (error) {
    const errorMessage = `Failed to start L Language Server: ${error}`;
    outputChannel.appendLine(`[ERROR] ${errorMessage}`);
    if (error instanceof Error) {
      outputChannel.appendLine(`[ERROR] Error stack: ${error.stack}`);
    }
    window.showErrorMessage(errorMessage);
    updateStatusBar(State.Stopped);
  }
}

/**
 * Called when the extension is deactivated.
 *
 * This function is called when the extension is unloaded. It stops the language client
 * and cleans up any resources.
 *
 * @returns A promise that resolves when deactivation is complete
 */
export function deactivate(): Thenable<void> | undefined {
  // Set disposing flag to prevent further UI updates
  isDisposing = true;

  // Clear any pending configuration change timer
  if (configChangeTimer) {
    clearTimeout(configChangeTimer);
    configChangeTimer = undefined;
    outputChannel.appendLine("[INFO] Cleared pending configuration change timer");
  }

  // If the client was never initialized, nothing to do
  if (!client) {
    outputChannel.appendLine("[INFO] No client to stop");
    return undefined;
  }

  outputChannel.appendLine("[INFO] Stopping L language server...");

  try {
    // Stop the language client and handle the result
    return client.stop().then(
      () => {
        outputChannel.appendLine("[INFO] L language server stopped successfully");
      },
      (reason) => {
        outputChannel.appendLine(`[ERROR] Failed to stop L language server: ${reason}`);
      },
    );
  } catch (error) {
    // Handle any unexpected errors during shutdown
    outputChannel.appendLine(`[ERROR] Error while stopping L language server: ${error}`);
    return Promise.resolve();
  }
}

/**
 * Update the status bar based on the client state.
 */
function updateStatusBar(state: State) {
  // Don't update status bar if extension is being disposed
  if (isDisposing) {
    return;
  }

  switch (state) {
    case State.Starting:
      statusBarItem.text = "$(sync~spin) L Language Server";
      statusBarItem.tooltip = "L Language Server is starting...";
      statusBarItem.color = undefined;
      break;
    case State.Running:
      statusBarItem.text = "$(check) L Language Server";
      statusBarItem.tooltip = "L Language Server is running";
      statusBarItem.color = undefined;
      break;
    case State.Stopped:
      statusBarItem.text = "$(x) L Language Server";
      statusBarItem.tooltip = "L Language Server has stopped";
      statusBarItem.color = new ThemeColor("statusBarItem.errorForeground");
      break;
  }
}

/**
 * Handle configuration changes.
 */
function handleConfigurationChange(event: ConfigurationChangeEvent) {
  if (event.affectsConfiguration("l-language-server")) {
    // Check if it's just a trace setting change (doesn't require restart)
    if (event.affectsConfiguration("l-language-server.trace.server")) {
      outputChannel.appendLine("[INFO] Trace configuration changed, no restart needed");
      return;
    }

    outputChannel.appendLine("[INFO] Configuration changed, scheduling server restart");

    // Clear existing timer if any
    if (configChangeTimer) {
      clearTimeout(configChangeTimer);
    }

    // Debounce: wait 1 second before restarting to avoid rapid restarts
    configChangeTimer = setTimeout(() => {
      restartServer();
      configChangeTimer = undefined;
    }, 1000);
  }
}

/**
 * Restart the language server.
 */
async function restartServer() {
  // Don't restart if extension is being disposed
  if (isDisposing) {
    outputChannel.appendLine("[WARN] Extension is being disposed, skipping restart");
    return;
  }

  if (!client) {
    outputChannel.appendLine("[WARN] Cannot restart server: client not initialized");
    return;
  }

  const currentState = client.state;
  outputChannel.appendLine(
    `[INFO] Restarting L Language Server (current state: ${State[currentState]})`,
  );

  try {
    // Show progress notification
    await window.withProgress(
      {
        location: ProgressLocation.Notification,
        title: "Restarting L Language Server",
        cancellable: false,
      },
      async () => {
        // Stop the client if it's running
        if (currentState === State.Running) {
          outputChannel.appendLine("[INFO] Stopping current server instance...");
          await client.stop();
          outputChannel.appendLine("[INFO] Server stopped successfully");
        } else if (currentState === State.Starting) {
          outputChannel.appendLine(
            "[WARN] Server is currently starting, waiting for it to complete...",
          );
          try {
            await new Promise<void>((resolve) => {
              const disposable = client.onDidChangeState((state) => {
                if (isDisposing) {
                  disposable.dispose();
                  resolve();
                  return;
                }
                if (state.newState === State.Running) {
                  disposable.dispose();
                  resolve();
                } else if (state.newState === State.Stopped) {
                  disposable.dispose();
                  resolve();
                }
              });
            });
            // Check state again after waiting
            if (client.state === State.Running) {
              await client.stop();
              outputChannel.appendLine("[INFO] Server stopped successfully");
            } else {
              outputChannel.appendLine(
                "[WARN] Server failed to start, proceeding with restart attempt",
              );
            }
          } catch (error) {
            outputChannel.appendLine(`[WARN] Server failed to start during wait: ${error}`);
          }
        }

        // Start the client
        outputChannel.appendLine("[INFO] Starting new server instance...");
        await client.start();

        // Wait for the client to be ready
        await new Promise<void>((resolve) => {
          const disposable = client.onDidChangeState((state) => {
            if (isDisposing) {
              disposable.dispose();
              resolve();
              return;
            }
            if (state.newState === State.Running) {
              outputChannel.appendLine("[INFO] L Language Server restarted successfully");
              disposable.dispose();
              resolve();
            } else if (state.newState === State.Stopped) {
              disposable.dispose();
              resolve();
              outputChannel.appendLine("[ERROR] L Language Server failed to restart");
            }
          });
        });
      },
    );
  } catch (error) {
    const errorMessage = `Failed to restart L Language Server: ${error}`;
    outputChannel.appendLine(`[ERROR] ${errorMessage}`);
    if (error instanceof Error) {
      outputChannel.appendLine(`[ERROR] Error stack: ${error.stack}`);
    }
    window.showErrorMessage(errorMessage);
  }
}
