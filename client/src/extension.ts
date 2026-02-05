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

import { ExtensionContext, window, workspace } from "vscode";

import {
  CloseAction,
  ErrorAction,
  Executable,
  LanguageClient,
  LanguageClientOptions,
  ServerOptions
} from "vscode-languageclient/node";

/**
 * The language client instance.
 * This is the main connection to the L language server.
 */
let client: LanguageClient;

/**
 * Called when the extension is activated.
 * 
 * This function is called when the extension is first loaded. It sets up the
 * language client and starts the connection to the L language server.
 * 
 * @param _context The extension context
 */
export async function activate(_context: ExtensionContext) {
  // Create an output channel for tracing LSP communication
  const traceOutputChannel = window.createOutputChannel("L Language Server trace");

  // Try to locate the server executable
  let serverCommand: string | undefined;
  const path = await import("path");
  const fs = await import("fs");
  const os = await import("os");

  // Check if a custom server path is specified in environment variables
  if (process.env.SERVER_PATH) {
    serverCommand = process.env.SERVER_PATH;
    console.log("Using server from SERVER_PATH:", serverCommand);
  } else {
    // Attempt to locate the server in multiple possible locations
    const possiblePaths = [
      // Relative to the extension's dist directory
      path.join(
        __dirname,
        "..",
        "..",
        "target",
        "debug",
        "l-language-server" + (os.platform() === "win32" ? ".exe" : ""),
      ),
      // Relative to the workspace root (when developing)
      path.join(
        __dirname,
        "..",
        "..",
        "..",
        "target",
        "debug",
        "l-language-server" + (os.platform() === "win32" ? ".exe" : ""),
      ),
      // Direct command (if installed globally)
      "l-language-server",
    ];

    console.log("Searching for server in possible paths:", possiblePaths);

    // Find the first path that exists
    for (const possiblePath of possiblePaths) {
      console.log("Checking path:", possiblePath);
      if (fs.existsSync(possiblePath)) {
        serverCommand = possiblePath;
        console.log("Found server at:", serverCommand);
        break;
      }
    }

    // If none of the paths worked, use the fallback
    if (!serverCommand) {
      serverCommand = "l-language-server";
      console.log("No local server found, using fallback command:", serverCommand);
    }
  }

  // Ensure the server command is properly formatted for the platform
  serverCommand = path.normalize(serverCommand || "l-language-server");
  console.log("Final server command:", serverCommand);

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
  let clientOptions: LanguageClientOptions = {
    // Register the server for plain text documents with the 'l' language
    documentSelector: [{ scheme: "file", language: "l" }],
    
    // Synchronize configuration and file changes with the server
    synchronize: {
      // Notify the server about file changes to '.clientrc files contained in the workspace
      fileEvents: workspace.createFileSystemWatcher("**/.clientrc"),
    },
    
    // Use the trace output channel for logging
    traceOutputChannel,
    
    // Error handling and reconnection options
    errorHandler: {
      // Handle errors that occur during LSP communication
      error: (error: Error, message: any, count: number) => {
        console.error(`LSP Error (${count}): ${error}`);
        console.error(`Message: ${message}`);
        return { action: ErrorAction.Continue }; // Continue running the server despite errors
      },
      // Handle when the LSP connection is closed
      closed: () => {
        console.error("LSP connection closed");
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
  
  // Start the language client
  client.start();
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
  // If the client was never initialized, nothing to do
  if (!client) {
    return undefined;
  }
  
  console.log("Stopping L language server...");
  
  try {
    // Stop the language client and handle the result
    return client.stop().then(
      () => {
        console.log("L language server stopped successfully");
      },
      (reason) => {
        console.error(`Failed to stop L language server: ${reason}`);
      }
    );
  } catch (error) {
    // Handle any unexpected errors during shutdown
    console.error(`Error while stopping L language server: ${error}`);
    return Promise.resolve();
  }
}