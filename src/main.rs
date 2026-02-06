//! L Language Server Implementation
//!
//! This module implements a Language Server Protocol (LSP) server for the L programming language.
//! It provides features such as code completion, goto definition, references, rename,
//! formatting, inlay hints, and semantic tokens.
//!
//! The server is built using the tower-lsp-server library and communicates with the client
//! through JSON-RPC messages.

use dashmap::DashMap;
use l_lang::{
    AstNode, CompileResult, Formatter, SymbolId, SymbolKind, Type, compile, find_node_at_offset,
};
use log::debug;
use ropey::Rope;
use serde_json::Value;

use std::str::FromStr;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::{
    CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams, CompletionResponse,
    Diagnostic, DidChangeConfigurationParams, DidChangeTextDocumentParams,
    DidChangeWatchedFilesParams, DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentFilter, DocumentFormattingParams,
    ExecuteCommandOptions, ExecuteCommandParams, GotoDefinitionParams, GotoDefinitionResponse,
    InitializeParams, InitializeResult, InitializedParams, InlayHint, InlayHintKind,
    InlayHintLabel, InlayHintLabelPart, InlayHintParams, Location, MessageType, OneOf, Position,
    Range, ReferenceParams, RenameParams, SaveOptions, SemanticToken, SemanticTokenType,
    SemanticTokens, SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions,
    SemanticTokensParams, SemanticTokensRangeParams, SemanticTokensRangeResult,
    SemanticTokensRegistrationOptions, SemanticTokensResult, SemanticTokensServerCapabilities,
    ServerCapabilities, StaticRegistrationOptions, TextDocumentRegistrationOptions,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, TextEdit, Uri, WorkDoneProgressOptions, WorkspaceEdit,
    WorkspaceFoldersServerCapabilities, WorkspaceServerCapabilities,
};
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

#[derive(Debug)]
/// The backend implementation for the L language server.
///
/// This struct maintains the state of the language server, including:
/// - Client connection for sending notifications and requests
/// - Document content mapping (URI -> Rope)
/// - Semantic analysis results mapping (URI -> CompileResult)
/// - Shutdown flag for graceful termination
struct Backend {
    /// The LSP client connection
    client: Client,
    /// Maps document URIs to their text content represented as Rope
    document_map: DashMap<String, Rope>,
    /// Maps document URIs to their semantic analysis results
    semanticast_map: DashMap<String, CompileResult>,
    /// Atomic flag indicating if the server is shutting down
    is_shutdown: std::sync::atomic::AtomicBool,
}

impl LanguageServer for Backend {
    /// Initialize the language server.
    ///
    /// This method is called by the client when the server is first connected.
    /// It returns the server capabilities, which inform the client about
    /// which features the server supports.
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        //  Ok(InitializeResult::default())
        Ok(InitializeResult {
            server_info: None,
            offset_encoding: None,

            capabilities: ServerCapabilities {
                document_formatting_provider: Some(OneOf::Left(true)),
                inlay_hint_provider: Some(OneOf::Left(true)),
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                        ..Default::default()
                    },
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string()]),
                    work_done_progress_options: Default::default(),
                    all_commit_characters: None,
                    completion_item: None,
                }),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["dummy.do_something".to_string()],
                    work_done_progress_options: Default::default(),
                }),

                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                        SemanticTokensRegistrationOptions {
                            text_document_registration_options: {
                                TextDocumentRegistrationOptions {
                                    document_selector: Some(vec![DocumentFilter {
                                        language: Some("l".to_string()),
                                        scheme: Some("file".to_string()),
                                        pattern: None,
                                    }]),
                                }
                            },
                            semantic_tokens_options: SemanticTokensOptions {
                                work_done_progress_options: WorkDoneProgressOptions::default(),
                                legend: SemanticTokensLegend {
                                    token_types: vec![
                                        SemanticTokenType::FUNCTION,
                                        SemanticTokenType::VARIABLE,
                                        SemanticTokenType::PARAMETER,
                                        SemanticTokenType::STRUCT,
                                        SemanticTokenType::PROPERTY,
                                    ],
                                    token_modifiers: vec![],
                                },
                                range: Some(true),
                                full: Some(SemanticTokensFullOptions::Bool(true)),
                            },
                            static_registration_options: StaticRegistrationOptions::default(),
                        },
                    ),
                ),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
        })
    }

    /// Notification that the client has finished initializing.
    ///
    /// This method is called after the client has received the result of the initialize request
    /// and the client is ready to send requests.
    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
        debug!("initialized!");
    }

    /// Shutdown the language server.
    ///
    /// This method is called by the client when it wants to shut down the server.
    /// The server should respond with Ok(()) and then exit.
    async fn shutdown(&self) -> Result<()> {
        debug!("Shutdown request received");

        // Set the shutdown flag
        self.is_shutdown
            .store(true, std::sync::atomic::Ordering::Release);

        // Clear all stored data to free resources
        self.semanticast_map.clear();
        self.document_map.clear();

        debug!(
            "Cleared {} documents and {} semantic results",
            self.document_map.len(),
            self.semanticast_map.len()
        );
        debug!("Server shutting down gracefully");
        Ok(())
    }

    /// Called when a document is opened in the client.
    ///
    /// This notification is sent from the client to the server when a document is opened.
    /// The server compiles the document and stores the results for later use.
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.on_change(TextDocumentChange {
            uri: params.text_document.uri.to_string(),
            text: &params.text_document.text,
        })
        .await;
        debug!("file opened!");
    }

    /// Called when the content of a document changes in the client.
    ///
    /// This notification is sent from the client to the server when a document is modified.
    /// The server recompiles the document and updates its internal state.
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // Check if content_changes is not empty to prevent panic
        if params.content_changes.is_empty() {
            debug!("Received empty content_changes, ignoring");
            return;
        }

        self.on_change(TextDocumentChange {
            text: &params.content_changes[0].text,
            uri: params.text_document.uri.to_string(),
        })
        .await;
    }

    /// Called when a document is saved in the client.
    ///
    /// This notification is sent from the client to the server when a document is saved.
    /// The server recompiles the document to ensure the saved version is analyzed.
    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        let text = if let Some(text) = params.text {
            text
        } else {
            // If no text provided, use the stored document content
            if let Some(rope) = self.document_map.get(&uri) {
                rope.to_string()
            } else {
                debug!("No stored content for document: {}", uri);
                return;
            }
        };

        self.on_change(TextDocumentChange { text: &text, uri })
            .await;
        debug!("file saved!");
    }

    /// Called when a document is closed in the client.
    ///
    /// This notification is sent from the client to the server when a document is closed.
    /// The server removes the document from its internal state to free resources.
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.document_map
            .remove(&params.text_document.uri.to_string());
        self.semanticast_map
            .remove(&params.text_document.uri.to_string());
        debug!("file closed!");
    }

    /// Go to the definition of the symbol at the given position.
    ///
    /// This request is sent from the client to the server to get the location
    /// of the definition of the symbol at the given cursor position.
    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();
        let position = params.text_document_position_params.position;
        debug!(
            "Goto definition request for {} at line {}, col {}",
            uri, position.line, position.character
        );

        let definition = self.get_definition(params);

        if definition.is_some() {
            debug!(
                "Found definition for symbol at line {}, col {}",
                position.line, position.character
            );
        } else {
            debug!(
                "No definition found for symbol at line {}, col {}",
                position.line, position.character
            );
        }

        Ok(definition)
    }

    /// Find all references to the symbol at the given position.
    ///
    /// This request is sent from the client to the server to get all locations
    /// where the symbol at the given cursor position is referenced.
    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;
        let include_declaration = params.context.include_declaration;
        debug!(
            "References request for {} at line {}, col {} (include_declaration: {})",
            uri, position.line, position.character, include_declaration
        );

        let references = self.get_references(uri.clone(), position, include_declaration);

        if let Some(refs) = &references {
            debug!(
                "Found {} references for symbol at line {}, col {}",
                refs.len(),
                position.line,
                position.character
            );
        } else {
            debug!(
                "No references found for symbol at line {}, col {}",
                position.line, position.character
            );
        }

        Ok(references)
    }

    /// Provide semantic tokens for the entire document.
    ///
    /// This request is sent from the client to the server to get semantic tokens,
    /// which are used for syntax highlighting based on semantic understanding.
    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri.to_string();
        let semantic_tokens = self.build_semantic_tokens(&uri);
        if let Some(tokens) = semantic_tokens {
            return Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
                result_id: None,
                data: tokens,
            })));
        }
        Ok(None)
    }

    /// Provide semantic tokens for a specific range in a document.
    ///
    /// This request is sent from the client to the server to get semantic tokens
    /// for a specific range, which is used for incremental syntax highlighting.
    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        let uri = params.text_document.uri.to_string();
        let range = params.range;
        let semantic_tokens = self.build_semantic_tokens_range(&uri, range);
        Ok(semantic_tokens.map(|data| {
            SemanticTokensRangeResult::Tokens(SemanticTokens {
                result_id: None,
                data,
            })
        }))
    }

    /// Provide inlay hints for a document.
    ///
    /// This request is sent from the client to the server to get inlay hints,
    /// which are additional information displayed inline with the code.
    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri.to_string();
        Ok(self.build_inlay_hints(&uri))
    }

    /// Provide code completion items at a specific position in a document.
    ///
    /// This request is sent from the client to the server to get completion items
    /// at a given cursor position. The server analyzes the context and provides
    /// relevant suggestions such as variables, functions, and fields.
    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let completions = self.get_completion(params);
        Ok(completions.map(CompletionResponse::Array))
    }

    /// Rename the symbol at the given position.
    ///
    /// This request is sent from the client to the server to rename the symbol
    /// at the given cursor position and all its references.
    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;
        let new_name = params.new_name.clone();
        debug!(
            "Rename request for {} at line {}, col {} to '{}'",
            uri, position.line, position.character, new_name
        );

        let workspace_edit = self.get_rename_edit(uri.clone(), position, new_name);

        if workspace_edit.is_some() {
            debug!("Created workspace edit for rename operation");
        } else {
            debug!("Could not create workspace edit for rename operation");
        }

        Ok(workspace_edit)
    }

    /// Format the entire document.
    ///
    /// This request is sent from the client to the server to format the entire document
    /// according to the language's formatting rules.
    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        Ok(self.format_text(params))
    }

    async fn did_change_configuration(&self, _: DidChangeConfigurationParams) {
        debug!("configuration changed!");
    }

    async fn did_change_workspace_folders(&self, _: DidChangeWorkspaceFoldersParams) {
        debug!("workspace folders changed!");
    }

    async fn did_change_watched_files(&self, _: DidChangeWatchedFilesParams) {
        debug!("watched files have changed!");
    }

    async fn execute_command(&self, _: ExecuteCommandParams) -> Result<Option<Value>> {
        debug!("command executed!");

        Ok(None)
    }
}

#[tokio::main]
/// Entry point for the L language server.
///
/// This function sets up the server, handles signals for graceful shutdown,
/// and starts the main event loop.
async fn main() {
    // Initialize logger
    env_logger::init();
    debug!("Starting L Language Server");

    // Set up signal handling for graceful shutdown
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    // Handle Ctrl+C signal
    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                debug!("Received shutdown signal (Ctrl+C)");
                let _ = shutdown_tx.send(());
            }
            Err(err) => {
                eprintln!("Unable to listen for shutdown signal: {}", err);
            }
        }
    });

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    debug!("Creating LSP service");
    let (service, socket) = LspService::build(|client| Backend {
        client,
        semanticast_map: DashMap::new(),
        document_map: DashMap::new(),
        is_shutdown: std::sync::atomic::AtomicBool::new(false),
    })
    .finish();

    debug!("Starting server with tokio::select! for graceful shutdown");
    let server = Server::new(stdin, stdout, socket).serve(service);

    tokio::select! {
        _ = server => {
            debug!("Server completed normally");
        }
        _ = &mut shutdown_rx => {
            debug!("Received shutdown signal, terminating server");
        }
    }
}

impl Backend {
    /// Check if the server is shutting down.
    ///
    /// This method checks the atomic shutdown flag to determine if the server
    /// is in the process of shutting down. This is used to avoid unnecessary work
    /// during shutdown.
    fn is_shutting_down(&self) -> bool {
        self.is_shutdown.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Convert SymbolKind to semantic token type.
    ///
    /// Token type indices correspond to LEGEND_TYPE order:
    /// 0: FUNCTION, 1: VARIABLE, 2: PARAMETER, 3: STRUCT, 4: PROPERTY (field)
    fn symbol_kind_to_token_type(&self, kind: SymbolKind) -> u32 {
        match kind {
            SymbolKind::Function => 0,
            SymbolKind::Variable => 1,
            SymbolKind::Parameter => 2,
            SymbolKind::Struct => 3,
            SymbolKind::Field => 4,
        }
    }

    /// Convert incomplete tokens to LSP SemanticToken format with delta encoding.
    ///
    /// This method takes a list of tokens with (start, length, token_type) and
    /// converts them to the LSP SemanticToken format with delta encoding.
    fn convert_to_semantic_tokens(
        &self,
        incomplete_tokens: Vec<(usize, usize, u32)>,
        rope: &Rope,
    ) -> Option<Vec<SemanticToken>> {
        let mut tokens = incomplete_tokens;
        tokens.sort_by(|a, b| a.0.cmp(&b.0));

        let mut pre_line: u32 = 0;
        let mut pre_start: u32 = 0;

        let semantic_tokens = tokens
            .iter()
            .filter_map(|(start, length, token_type)| {
                let line = rope.try_byte_to_line(*start).ok()? as u32;
                let line_start_byte = rope.try_line_to_byte(line as usize).ok()?;
                let char_offset = *start - line_start_byte;

                let delta_line = line - pre_line;
                let delta_start = if delta_line == 0 {
                    char_offset as u32 - pre_start
                } else {
                    char_offset as u32
                };

                let token = SemanticToken {
                    delta_line,
                    delta_start,
                    length: *length as u32,
                    token_type: *token_type,
                    token_modifiers_bitset: 0,
                };

                pre_line = line;
                pre_start = char_offset as u32;

                Some(token)
            })
            .collect::<Vec<_>>();

        Some(semantic_tokens)
    }
    /// Format the text of a document.
    ///
    /// This method uses the l_lang formatter to format the entire document
    /// and returns the text edits needed to apply the formatting.
    fn format_text(&self, params: DocumentFormattingParams) -> Option<Vec<TextEdit>> {
        let uri = params.text_document.uri.to_string();
        let rope = self.document_map.get(&uri)?;
        let semantic_result = self.semanticast_map.get(&uri)?;
        let formatter = Formatter::new(80);
        let formatted_text = formatter.format(semantic_result.program.file(), &rope.to_string());
        Some(vec![TextEdit {
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(
                    rope.len_lines() as u32,
                    rope.line(rope.len_lines() - 1).len_chars() as u32,
                ),
            },
            new_text: formatted_text,
        }])
    }

    /// Build inlay hints for a document.
    ///
    /// This method analyzes the semantic information of a document and creates
    /// inlay hints for variable types and other useful information.
    fn build_inlay_hints(&self, uri: &str) -> Option<Vec<InlayHint>> {
        let semantic_result = self.semanticast_map.get(uri)?;
        let rope = self.document_map.get(uri)?;
        let bindings = &semantic_result.semantic.bindings;
        let hints = bindings
            .iter_enumerated()
            .filter_map(|(symbol_id, type_info)| {
                if semantic_result.semantic.get_symbol_kind(symbol_id)
                    != l_lang::SymbolKind::Variable
                {
                    return None;
                }
                // Get the symbol definition span (not the binding span)
                let symbol_span = semantic_result.semantic.symbol_spans.get(symbol_id)?;
                let end = offset_to_position(symbol_span.end as usize, &rope)?;
                let inlay_hint_parts = match type_info.ty {
                    Type::Struct(id) => {
                        let mut parts = vec![];
                        parts.push(InlayHintLabelPart {
                            value: ": ".to_string(),
                            ..Default::default()
                        });
                        let span = semantic_result.semantic.get_symbol_span(id);
                        let start = offset_to_position(span.start as usize, &rope)?;
                        let end = offset_to_position(span.end as usize, &rope)?;
                        // For LSP URIs, we need to parse them correctly
                        if let Ok(uri_obj) = Uri::from_str(uri) {
                            let location = Location::new(uri_obj, Range::new(start, end));
                            parts.push(InlayHintLabelPart {
                                value: type_info.ty.format_literal_type(&semantic_result.semantic),
                                location: Some(location),
                                ..Default::default()
                            });
                        } else {
                            parts.push(InlayHintLabelPart {
                                value: type_info.ty.format_literal_type(&semantic_result.semantic),
                                location: None,
                                ..Default::default()
                            });
                        }
                        InlayHintLabel::LabelParts(parts)
                    }
                    _ => InlayHintLabel::String(format!(
                        ": {}",
                        type_info.ty.format_literal_type(&semantic_result.semantic)
                    )),
                };
                Some(InlayHint {
                    position: Position::new(end.line, end.character),
                    label: inlay_hint_parts,
                    kind: Some(InlayHintKind::TYPE),
                    text_edits: None,
                    tooltip: None,
                    padding_left: Some(true),
                    padding_right: Some(false),
                    data: None,
                })
            })
            .collect::<Vec<_>>();

        Some(hints)
    }

    /// Get the definition location for a symbol at a given position.
    ///
    /// This method finds the symbol at the given position and returns
    /// the location of its definition.
    fn get_definition(&self, params: GotoDefinitionParams) -> Option<GotoDefinitionResponse> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();
        let position = params.text_document_position_params.position;

        let rope = self.document_map.get(&uri)?;

        let compilation_result = self.semanticast_map.get(&uri)?;
        let offset = position_to_offset(position, &rope)?;

        // First check if cursor is on a reference (not a definition)
        if let Some(interval) = compilation_result
            .semantic
            .span_to_reference
            .find(offset, offset + 1)
            .next()
        {
            let ref_id = interval.val;

            // Check if ref_id is within bounds
            if ref_id >= compilation_result.semantic.references.len() {
                return None;
            }

            let symbol_id = compilation_result.semantic.references[ref_id]?;
            let symbol_span = compilation_result.semantic.get_symbol_span(symbol_id);
            let start = offset_to_position(symbol_span.start as usize, &rope)?;
            let end = offset_to_position(symbol_span.end as usize, &rope)?;
            let location = Location::new(
                params
                    .text_document_position_params
                    .text_document
                    .uri
                    .clone(),
                Range::new(start, end),
            );
            return Some(GotoDefinitionResponse::Scalar(location));
        }

        // If not on a reference, check if cursor is on a symbol definition
        if let Some(interval) = compilation_result
            .semantic
            .span_to_symbol
            .find(offset, offset + 1)
            .next()
        {
            // Skip if interval is invalid
            if interval.start >= interval.stop {
                return None;
            }
            let start = offset_to_position(interval.start, &rope)?;
            let end = offset_to_position(interval.stop, &rope)?;
            let location = Location::new(
                params
                    .text_document_position_params
                    .text_document
                    .uri
                    .clone(),
                Range::new(start, end),
            );
            return Some(GotoDefinitionResponse::Scalar(location));
        }

        None
    }

    /// Get all references to a symbol at a given position.
    ///
    /// This method finds the symbol at the given position and returns
    /// all locations where this symbol is referenced.
    fn get_references(
        &self,
        uri: String,
        position: Position,
        include_declaration: bool,
    ) -> Option<Vec<Location>> {
        let rope = self.document_map.get(&uri)?;
        let compilation_result = self.semanticast_map.get(&uri)?;
        let offset = position_to_offset(position, &rope)?;
        let symbol_id = compilation_result.semantic.get_symbol_at(offset);
        let symbol_id = symbol_id?;

        let mut references = Vec::new();
        // Parse the URI string into a Uri object
        if let Ok(uri_obj) = Uri::from_str(&uri) {
            if include_declaration {
                // Include the symbol definition itself
                let symbol_span = compilation_result.semantic.get_symbol_span(symbol_id);
                let start = offset_to_position(symbol_span.start as usize, &rope)?;
                let end = offset_to_position(symbol_span.end as usize, &rope)?;
                references.push(Location::new(uri_obj.clone(), Range::new(start, end)));
            }
            // Find the reference at the current position
            let ref_ids = compilation_result.semantic.get_symbol_references(symbol_id);

            references.extend(ref_ids.iter().filter_map(|ref_id| {
                // Check if ref_id is within bounds
                if *ref_id >= compilation_result.semantic.reference_spans.len() {
                    return None;
                }

                let span = compilation_result.semantic.reference_spans[*ref_id];
                let start = offset_to_position(span.start as usize, &rope)?;
                let end = offset_to_position(span.end as usize, &rope)?;
                Some(Location::new(uri_obj.clone(), Range::new(start, end)))
            }));
        }
        Some(references)
    }

    /// Create a workspace edit for renaming a symbol.
    ///
    /// This method finds all references to the symbol at the given position
    /// and creates a workspace edit that replaces them with the new name.
    fn get_rename_edit(
        &self,
        uri: String,
        position: Position,
        new_name: String,
    ) -> Option<WorkspaceEdit> {
        let all_reference = self.get_references(uri.clone(), position, true)?;

        let edits = all_reference
            .into_iter()
            .map(|item| TextEdit {
                range: item.range,
                new_text: new_name.clone(),
            })
            .collect::<Vec<_>>();

        // Create workspace edit with the text edits
        // Parse the URI string into a Uri object
        if let Ok(parsed_uri) = Uri::from_str(&uri) {
            let mut edit_map = std::collections::HashMap::new();
            edit_map.insert(parsed_uri, edits);

            Some(WorkspaceEdit::new(edit_map))
        } else {
            None
        }
    }

    /// Get the struct ID from a field access expression.
    ///
    /// This method traverses the field access chain to find the base struct
    /// and returns its symbol ID.
    fn get_struct_id_from_field(
        &self,
        field_expr: &l_lang::ExprField,
        semantic_result: &CompileResult,
    ) -> Option<SymbolId> {
        let mut access_arr = vec![];
        let mut cur = field_expr.object.as_ref()?;
        loop {
            match cur.as_ref() {
                l_lang::Expr::Field(field_expr) => {
                    access_arr.push(field_expr.field.as_ref()?.name.clone());
                    cur = field_expr.object.as_ref()?;
                }
                l_lang::Expr::Name(_name_expr) => {
                    break;
                }
                _ => {
                    return None;
                }
            }
        }
        access_arr.reverse();

        let object_span = field_expr.object.as_ref()?.span();
        let reference_id = semantic_result
            .semantic
            .get_reference_at(object_span.start as usize)?;

        // Check if reference_id is within bounds
        if reference_id >= semantic_result.semantic.references.len() {
            return None;
        }

        let symbol_id = semantic_result.semantic.references[reference_id]?;
        let ty_info = semantic_result.semantic.get_symbol_type(symbol_id)?;
        let Type::Struct(mut struct_id) = ty_info.ty else {
            return None;
        };

        for field_name in access_arr {
            let struct_def = semantic_result.semantic.structs.get(&struct_id)?;
            let field = struct_def.fields.iter().find(|f| f.name == field_name)?;
            let Type::Struct(next_struct_id) = field.ty else {
                return None;
            };
            struct_id = next_struct_id;
        }
        Some(struct_id)
    }

    /// Get completion items for a given position.
    ///
    /// This method analyzes the context at the given position and provides
    /// relevant completion items such as variables, functions, and fields.
    fn get_completion(&self, params: CompletionParams) -> Option<Vec<CompletionItem>> {
        let text_doc_position = params.text_document_position;
        let uri = text_doc_position.text_document.uri.to_string();
        let semantic_result = self.semanticast_map.get(&uri)?;
        let rope = self.document_map.get(&uri)?;
        let offset = position_to_offset(text_doc_position.position, &rope)?;

        let mut items = Vec::new();

        // Helper function to create completion items from symbols
        let create_symbol_completions = |semantic_result: &CompileResult, rope: &Rope| {
            let bindings = &semantic_result.semantic.bindings;
            bindings
                .iter_enumerated()
                .filter_map(|(symbol_id, type_info)| {
                    let symbol_kind = semantic_result.semantic.get_symbol_kind(symbol_id);
                    let span = semantic_result.semantic.get_symbol_span(symbol_id);

                    // Check if span is valid
                    if span.start >= span.end {
                        return None;
                    }

                    let name_slice = rope.byte_slice(span.start as usize..span.end as usize);
                    if let Ok(name) =
                        std::str::from_utf8(name_slice.bytes().collect::<Vec<_>>().as_slice())
                    {
                        let (kind, detail) = match symbol_kind {
                            l_lang::SymbolKind::Variable => (
                                Some(CompletionItemKind::VARIABLE),
                                Some(format!(
                                    ": {}",
                                    type_info.ty.format_literal_type(&semantic_result.semantic)
                                )),
                            ),
                            l_lang::SymbolKind::Function => {
                                (Some(CompletionItemKind::FUNCTION), None)
                            }
                            l_lang::SymbolKind::Struct => (Some(CompletionItemKind::STRUCT), None),
                            _ => (None, None),
                        };

                        Some(CompletionItem {
                            label: name.to_string(),
                            kind,
                            detail,
                            insert_text: Some(name.to_string()),
                            ..Default::default()
                        })
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };

        // Try to find the AST node at the current position
        if let Some(nearest_node) =
            find_node_at_offset(semantic_result.program.file(), offset as u32)
        {
            match nearest_node {
                // Field access completion: suggest available fields/members
                AstNode::ExprField(field_expr) => {
                    let struct_id = self.get_struct_id_from_field(field_expr, &semantic_result)?;
                    let struct_def = semantic_result.semantic.structs.get(&struct_id)?;
                    struct_def.fields.iter().for_each(|field| {
                        items.push(CompletionItem {
                            label: field.name.clone(),
                            kind: Some(CompletionItemKind::FIELD),
                            detail: Some(format!(
                                ": {}",
                                field.ty.format_literal_type(&semantic_result.semantic)
                            )),
                            insert_text: Some(field.name.clone()),
                            ..Default::default()
                        });
                    });
                }
                _ => {
                    // Default: suggest all available symbols
                    items.extend(create_symbol_completions(&semantic_result, &rope));
                }
            }
        } else {
            // No node found, suggest all available symbols
            items.extend(create_symbol_completions(&semantic_result, &rope));
        }
        Some(items)
    }

    /// Handle a document change event.
    ///
    /// This method is called when a document is opened, changed, or saved.
    /// It compiles the document and publishes diagnostics.
    async fn on_change(&self, item: TextDocumentChange<'_>) {
        debug!("Processing document change for: {}", item.uri);

        let rope = Rope::from_str(item.text);
        debug!(
            "Created rope with {} lines and {} chars",
            rope.len_lines(),
            rope.len_chars()
        );

        let compile_result = compile(item.text);
        debug!(
            "Compilation completed with {} diagnostics and {} semantic errors",
            compile_result.diagnostics.len(),
            compile_result.semantic.errors.len()
        );

        let mut diagnostics = compile_result
            .diagnostics
            .iter()
            .flat_map(|d| {
                d.labels.iter().filter_map(|label| {
                    let start = offset_to_position(label.range.start, &rope)?;
                    let end = offset_to_position(label.range.end, &rope)?;
                    let diag = Diagnostic {
                        range: Range::new(start, end),
                        severity: None,
                        code: None,
                        code_description: None,
                        source: None,
                        message: format!("{:?}", d.message),
                        related_information: None,
                        tags: None,
                        data: None,
                    };
                    Some(diag)
                })
            })
            .collect::<Vec<_>>();

        compile_result.semantic.errors.iter().for_each(|sem_err| {
            let span = sem_err.span;
            let start = offset_to_position(span.start as usize, &rope);
            let end = offset_to_position(span.end as usize, &rope);
            if let (Some(start), Some(end)) = (start, end) {
                let diag = Diagnostic {
                    range: Range::new(start, end),
                    severity: None,
                    code: None,
                    code_description: None,
                    source: None,
                    message: sem_err.message.to_string(),
                    related_information: None,
                    tags: None,
                    data: None,
                };
                diagnostics.push(diag);
            }
        });

        debug!("Processed {} total diagnostics", diagnostics.len());

        // Check if the server is shutting down
        if self.is_shutting_down() {
            debug!("Skipping diagnostics publish - server is shutting down");
            return;
        }

        debug!(
            "Publishing {} diagnostics for document: {}",
            diagnostics.len(),
            item.uri
        );

        // Parse the URI string into a Uri object
        if let Ok(uri) = Uri::from_str(&item.uri) {
            // Double-check server status before publishing diagnostics
            if !self.is_shutting_down() {
                // publish_diagnostics returns () instead of Result, so call directly
                self.client
                    .publish_diagnostics(uri, diagnostics, None)
                    .await;
                debug!("Diagnostics published successfully");
            } else {
                debug!("Skipping diagnostics publish - server is shutting down");
            }
        } else {
            debug!("Failed to parse URI: {}", item.uri);
        }
        self.semanticast_map
            .insert(item.uri.clone(), compile_result);
        self.document_map.insert(item.uri.clone(), rope);
    }

    /// Build semantic tokens for an entire document.
    ///
    /// This method analyzes the semantic information of a document and creates
    /// semantic tokens for syntax highlighting based on symbol types.
    fn build_semantic_tokens(&self, uri: &str) -> Option<Vec<SemanticToken>> {
        let semantic_result = self.semanticast_map.get(uri)?;
        let rope = self.document_map.get(uri)?;

        // Collect all tokens from symbols and references
        // Token type indices correspond to LEGEND_TYPE order:
        // 0: FUNCTION, 1: VARIABLE, 2: PARAMETER, 3: STRUCT, 4: PROPERTY (field)
        let mut incomplete_tokens: Vec<(usize, usize, u32)> = Vec::new(); // (start, length, token_type)

        // Add symbol definitions
        for (symbol_id, span) in semantic_result.semantic.symbol_spans.iter_enumerated() {
            let kind = semantic_result.semantic.get_symbol_kind(symbol_id);
            let token_type = self.symbol_kind_to_token_type(kind);
            incomplete_tokens.push((
                span.start as usize,
                (span.end - span.start) as usize,
                token_type,
            ));
        }

        // Add references (they reference symbols, so use the symbol's kind)
        for (ref_id, span) in semantic_result.semantic.reference_spans.iter_enumerated() {
            // Check if ref_id is within bounds
            if ref_id >= semantic_result.semantic.references.len() {
                continue;
            }

            if let Some(symbol_id) = semantic_result.semantic.references[ref_id] {
                let kind = semantic_result.semantic.get_symbol_kind(symbol_id);
                let token_type = self.symbol_kind_to_token_type(kind);
                incomplete_tokens.push((
                    span.start as usize,
                    (span.end - span.start) as usize,
                    token_type,
                ));
            }
        }

        self.convert_to_semantic_tokens(incomplete_tokens, &rope)
    }

    /// Build semantic tokens for a specific range in a document.
    ///
    /// This method analyzes the semantic information of a document and creates
    /// semantic tokens for syntax highlighting within the specified range.
    fn build_semantic_tokens_range(&self, uri: &str, range: Range) -> Option<Vec<SemanticToken>> {
        let semantic_result = self.semanticast_map.get(uri)?;
        let rope = self.document_map.get(uri)?;

        // Convert range to byte offsets
        let start_offset = position_to_offset(range.start, &rope)?;
        let end_offset = position_to_offset(range.end, &rope)?;

        // Collect all tokens from symbols and references within the range
        let mut incomplete_tokens: Vec<(usize, usize, u32)> = Vec::new();

        // Add symbol definitions within range
        for (symbol_id, span) in semantic_result.semantic.symbol_spans.iter_enumerated() {
            let token_start = span.start as usize;
            if token_start >= start_offset && token_start < end_offset {
                let kind = semantic_result.semantic.get_symbol_kind(symbol_id);
                let token_type = self.symbol_kind_to_token_type(kind);
                incomplete_tokens.push((token_start, (span.end - span.start) as usize, token_type));
            }
        }

        // Add references within range
        for (ref_id, span) in semantic_result.semantic.reference_spans.iter_enumerated() {
            let token_start = span.start as usize;
            if token_start >= start_offset
                && token_start < end_offset
                && ref_id < semantic_result.semantic.references.len()
                && let Some(symbol_id) = semantic_result.semantic.references[ref_id]
            {
                let kind = semantic_result.semantic.get_symbol_kind(symbol_id);
                let token_type = self.symbol_kind_to_token_type(kind);
                incomplete_tokens.push((token_start, (span.end - span.start) as usize, token_type));
            }
        }

        self.convert_to_semantic_tokens(incomplete_tokens, &rope)
    }
}

/// Represents a change to a text document.
///
/// This struct contains the URI of the document and the new text content.
struct TextDocumentChange<'a> {
    /// The URI of the document
    uri: String,
    /// The new text content of the document
    text: &'a str,
}

/// Convert a byte offset to a position in the document.
///
/// This function converts a byte offset to a line and character position,
/// which is used by the LSP protocol.
fn offset_to_position(offset: usize, rope: &Rope) -> Option<Position> {
    // Check if offset is within rope bounds
    if offset > rope.len_chars() {
        return None;
    }

    // Handle the case where offset is exactly at the end of the file
    if offset == rope.len_chars() {
        let line = rope.len_lines() - 1;
        let column = rope.line(line).len_chars();
        return Some(Position::new(line as u32, column as u32));
    }

    let line = rope.try_char_to_line(offset).ok()?;
    let first_char_of_line = rope.try_line_to_char(line).ok()?;
    let column = offset - first_char_of_line;
    Some(Position::new(line as u32, column as u32))
}

/// Convert a position in the document to a byte offset.
///
/// This function converts a line and character position to a byte offset,
/// which is used internally for processing.
fn position_to_offset(position: Position, rope: &Rope) -> Option<usize> {
    // Check if line is within rope bounds
    let line = position.line as usize;
    if line >= rope.len_lines() {
        return None;
    }

    let line_char_offset = rope.try_line_to_char(line).ok()?;
    let line_len = rope.line(line).len_chars();

    // Handle the case where character is at or beyond the end of the line
    let char_offset = if position.character as usize >= line_len {
        line_len
    } else {
        position.character as usize
    };

    let total_offset = line_char_offset + char_offset;

    let slice = rope.slice(0..total_offset);
    Some(slice.len_bytes())
}
