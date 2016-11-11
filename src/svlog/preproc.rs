// Copyright (c) 2016 Fabian Schuiki

//! A preprocessor for SystemVerilog files that takes the raw stream of
//! tokens generated by a lexer and performs include and macro
//! resolution.

use std::path::Path;
use errors::{DiagResult2, DiagBuilder2};
use std::collections::HashMap;
use svlog::cat::*;
use source::*;
use std::rc::Rc;


type TokenAndSpan = (CatTokenKind, Span);

pub struct Preprocessor<'a> {
	/// The stack of input files. Tokens are taken from the topmost stream until
	/// the end of input, at which point the stream is popped and the process
	/// continues with the next stream. Used to handle include files.
	stack: Vec<Stream<'a>>,
	/// References to the source contents that were touched by the preprocessor.
	/// Keeping these around ensures that all emitted tokens remain valid (and
	/// point to valid memory locations) at least until the preprocessor is
	/// dropped.
	contents: Vec<Rc<SourceContent>>,
	/// The current token, or None if either the end of the stream has been
	/// encountered, or at the beginning when no token has been read yet.
	token: Option<TokenAndSpan>,
	/// The defined macros.
	macro_defs: HashMap<String, Macro>,
	/// The stack used to inject expanded macros into the token stream.
	macro_stack: Vec<TokenAndSpan>,
	/// The paths that are searched for included files, besides the current
	/// file's directory.
	include_paths: &'a [&'a Path],
	/// The define conditional stack. Whenever a `ifdef, `ifndef, `else, `elsif,
	/// or `endif directive is encountered, the stack is expanded, modified, or
	/// reduced to reflect the kind of conditional block we're in.
	defcond_stack: Vec<Defcond>,
}

impl<'a> Preprocessor<'a> {
	/// Create a new preprocessor for the given source file.
	pub fn new(source: Source, include_paths: &'a [&'a Path]) -> Preprocessor<'a> {
		let content = source.get_content();
		let content_unbound = unsafe { &*(content.as_ref() as *const SourceContent) };
		let iter = content_unbound.iter();
		Preprocessor {
			stack: vec![Stream {
				source: source,
				iter: Cat::new(iter)
			}],
			contents: vec![content],
			token: None,
			macro_defs: HashMap::new(),
			macro_stack: Vec::new(),
			include_paths: include_paths,
			defcond_stack: Vec::new(),
		}
	}

	/// Advance to the next token in the input stream.
	fn bump(&mut self) {
		self.token = self.macro_stack.pop();
		if self.token.is_some() {
			return
		}
		loop {
			self.token = match self.stack.last_mut() {
				Some(stream) => stream.iter.next().map(|tkn| (tkn.0, Span::new(stream.source, tkn.1, tkn.2))),
				None => return,
			};
			if self.token.is_none() {
				self.stack.pop();
			} else {
				break;
			}
		}
	}

	fn handle_directive<S: AsRef<str>>(&mut self, dir_name: S, span: Span) -> DiagResult2<()> {
		let dir_name = dir_name.as_ref();
		let dir = DIRECTIVES_TABLE.with(|tbl| tbl.get(dir_name).map(|x| *x).unwrap_or(Directive::Unknown));

		match dir {
			Directive::Include => {
				if self.is_inactive() {
					return Ok(());
				}

				// Skip leading whitespace.
				self.bump();
				match self.token {
					Some((Whitespace, _)) => self.bump(),
					_ => ()
				}

				// Match the opening double quotes or angular bracket.
				let name_p;
				let name_q;
				let closing = match self.token {
					Some((Symbol('"'), sp)) => { name_p = sp.end(); self.bump(); '"' },
					Some((Symbol('<'), sp)) => { name_p = sp.end(); self.bump(); '>' },
					_ => { return Err(DiagBuilder2::fatal("Expected filename inside double quotes (\"...\") or angular brackets (<...>) after `include").span(span))}
				};

				// Accumulate the include path until the closing symbol.
				let mut filename = String::new();
				loop {
					match self.token {
						Some((Symbol(c), sp)) if c == closing => {
							name_q = sp.begin();
							break;
						},
						Some((Newline, sp)) => {
							return Err(DiagBuilder2::fatal("Expected end of included file's name before line break").span(sp));
						},
						Some((_, sp)) => {
							filename.push_str(&sp.extract());
							self.bump();
						},
						None => {
							return Err(DiagBuilder2::fatal("Expected filename after `include directive before the end of the input").span(span));
						}
					}
				}

				// Create a new lexer for the included filename and push it onto the
				// stream stack.
				// TODO: Search only system location if `include <...> is used
				let included_source = match self.open_include(&filename, &span.source.get_path()) {
					Some(src) => src,
					None => {
						return Err(
							DiagBuilder2::fatal(format!("Cannot open included file \"{}\"", filename))
							.span(Span::union(name_p, name_q))
						);
					}
				};

				let content = included_source.get_content();
				let content_unbound = unsafe { &*(content.as_ref() as *const SourceContent) };
				let iter = content_unbound.iter();
				self.contents.push(content);
				self.stack.push(Stream {
					source: included_source,
					iter: Cat::new(iter)
				});

				return Ok(());
			}

			Directive::Define => {
				if self.is_inactive() {
					return Ok(());
				}

				// Skip leading whitespace.
				self.bump();
				match self.token {
					Some((Whitespace, _)) => self.bump(),
					_ => ()
				}

				// Consume the macro name.
				let (name, name_span) = match self.token {
					Some((Text, sp)) => (sp.extract(), sp),
					_ => return Err(DiagBuilder2::fatal("Expected macro name after `define").span(span))
				};
				self.bump();
				let mut makro = Macro::new(name.clone(), name_span);

				// NOTE: No whitespace is allowed after the macro name such that
				// the preprocessor does not mistake the a in "`define FOO (a)"
				// for a macro argument.

				// Consume the macro arguments and parameters.
				match self.token {
					Some((Symbol('('), _)) => {
						self.bump();
						loop {
							// Skip whitespace.
							match self.token {
								Some((Whitespace, _)) => self.bump(),
								Some((Symbol(')'), _)) => break,
								_ => ()
							}

							// Consume the argument name.
							let (name, name_span) = match self.token {
								Some((Text, sp)) => (sp.extract(), sp),
								_ => return Err(DiagBuilder2::fatal("Expected macro argument").span(span))
							};
							self.bump();
							makro.args.push(MacroArg::new(name, name_span));
							// TODO: Support default parameters.

							// Skip whitespace and either consume the comma that
							// follows or break out of the loop if a closing
							// parenthesis is encountered.
							match self.token {
								Some((Whitespace, _)) => self.bump(),
								_ => ()
							}
							match self.token {
								Some((Symbol(','), _)) => self.bump(),
								Some((Symbol(')'), _)) => break,
								Some((_, sp)) => return Err(DiagBuilder2::fatal("Expected , or ) after macro argument name").span(sp)),
								None => return Err(DiagBuilder2::fatal("Expected closing parenthesis at the end of the macro definition").span(span)),
							}
						}
						self.bump();
					},
					_ => ()
				}

				// Skip whitespace between the macro parameters and definition.
				match self.token {
					Some((Whitespace, _)) => self.bump(),
					_ => ()
				}

				// Consume the macro definition up to the next newline not preceded
				// by a backslash, ignoring comments, whitespace and newlines.
				loop {
					match self.token {
						Some((Newline, _)) => { break; },
						// Some((Whitespace, _)) => self.bump(),
						// Some((Comment, _)) => self.bump(),
						Some((Symbol('\\'), _)) => {
							self.bump();
							match self.token {
								Some((Newline, _)) => self.bump(),
								_ => ()
							};
						},
						Some(x) => {
							makro.body.push(x);
							self.bump();
						},
						None => break,
					}
				}

				self.macro_defs.insert(name, makro);
				return Ok(());
			}

			Directive::Ifdef | Directive::Ifndef | Directive::Elsif => {
				// Skip leading whitespace.
				self.bump();
				match self.token {
					Some((Whitespace, _)) => self.bump(),
					_ => ()
				}

				// Consume the macro name.
				let name = match self.token {
					Some((Text, sp)) => sp.extract(),
					_ => return Err(DiagBuilder2::fatal("Expected macro name after `ifdef").span(span))
				};
				let exists = self.macro_defs.contains_key(&name);

				// Depending on the directive, modify the define conditional
				// stack.
				match dir {
					Directive::Ifdef =>
						self.defcond_stack.push(if exists {
							Defcond::Enabled
						} else {
							Defcond::Disabled
						}),
					Directive::Ifndef =>
						self.defcond_stack.push(if exists {
							Defcond::Disabled
						} else {
							Defcond::Enabled
						}),
					Directive::Elsif => {
						match self.defcond_stack.pop() {
							Some(Defcond::Done) |
							Some(Defcond::Enabled) => self.defcond_stack.push(Defcond::Done),
							Some(Defcond::Disabled) => {
								if exists {
									self.defcond_stack.push(Defcond::Enabled);
								} else {
									self.defcond_stack.push(Defcond::Disabled);
								}
							},
							None => return Err(DiagBuilder2::fatal("Found `elsif without any preceeding `ifdef, `ifndef, or `elsif directive").span(span))
						};
					},
					_ => unreachable!(),
				}

				return Ok(());
			}

			Directive::Else => {
				match self.defcond_stack.pop() {
					Some(Defcond::Disabled) => self.defcond_stack.push(Defcond::Enabled),
					Some(Defcond::Enabled) | Some(Defcond::Done) => self.defcond_stack.push(Defcond::Done),
					None => return Err(DiagBuilder2::fatal("Found `else without any preceeding `ifdef, `ifndef, or `elsif directive").span(span))
				}
				return Ok(());
			}

			Directive::Endif => {
				if self.defcond_stack.pop().is_none() {
					return Err(DiagBuilder2::fatal("Found `endif without any preceeding `ifdef, `ifndef, `else, or `elsif directive").span(span));
				}
				return Ok(());
			}

			// Perform macro substitution. If we're currently inside the
			// inactive region of a define conditional (i.e. disabled or done),
			// don't bother expanding the macro.
			Directive::Unknown => {
				if self.is_inactive() {
					return Ok(());
				}
				if let Some(ref makro) = unsafe { &*(self as *const Preprocessor) }.macro_defs.get(dir_name) {

					// Consume the macro parameters if the macro definition
					// contains them.
					let mut params = HashMap::<String, Vec<TokenAndSpan>>::new();
					let mut args = makro.args.iter();
					if !makro.args.is_empty() {
						// Skip whitespace.
						self.bump();
						match self.token {
							Some((Whitespace, _)) => self.bump(),
							_ => ()
						}

						// Consume the opening paranthesis.
						match self.token {
							Some((Symbol('('), _)) => self.bump(),
							_ => return Err(DiagBuilder2::fatal("Expected macro parameters in parentheses '(...)'").span(span)),
						}

						// Consume the macro parameters.
						'outer: loop {
							// // Skip whitespace and break out of the loop if the
							// // closing parenthesis was encountered.
							// match self.token {
							// 	Some((Whitespace, _)) => self.bump(),
							// 	_ => ()
							// }
							// match self.token {
							// 	Some((Symbol(')'), _)) => break,
							// 	_ => ()
							// }

							// Fetch the next argument.
							let arg = match args.next() {
								Some(arg) => arg,
								None => return Err(DiagBuilder2::fatal("Superfluous macro parameters")),
							};

							// Consume the tokens that make up this argument.
							// Take care that it is allowed to have parentheses
							// as macro parameters, which requires bookkeeping
							// of the parentheses nesting level. If a comma is
							// encountered, we break out of the inner loop such
							// that the next parameter will be read. If a
							// closing parenthesis is encountered, we break out
							// of the outer loop to finish parameter parsing.
							let mut param_tokens = Vec::<TokenAndSpan>::new();
							let mut nesting = 0;
							loop {
								match self.token {
									// Some((Whitespace, _)) => self.bump(),
									// Some((Newline, _)) => self.bump(),
									// Some((Comment, _)) => self.bump(),
									Some((Symbol(','), _)) if nesting == 0 => {
										self.bump();
										params.insert(arg.name.clone(), param_tokens);
										break;
									},
									Some((Symbol(')'), _)) if nesting == 0 => {
										params.insert(arg.name.clone(), param_tokens);
										break 'outer;
									},
									Some((Symbol('('), _)) => {
										self.bump();
										nesting += 1;
									},
									Some((Symbol(')'), _)) if nesting > 0 => {
										self.bump();
										nesting -= 1;
									},
									Some(x) => {
										param_tokens.push(x);
										self.bump();
									},
									None => return Err(DiagBuilder2::fatal("Expected closing parenthesis after macro parameters").span(span)),
								}
							}
						}
					}

					// Push the tokens of the macro onto the stack, potentially
					// substituting any macro parameters as necessary.
					if params.is_empty() {
						self.macro_stack.extend(makro.body.iter().rev());
					} else {
						let mut replacement = Vec::<TokenAndSpan>::new();
						for tkn in &makro.body {
							match *tkn {
								(Text, sp) => {
									match params.get(&sp.extract()) {
										Some(substitute) => {
											replacement.extend(substitute);
										},
										None => replacement.push(*tkn)
									}
								},
								x => replacement.push(x)
							}
						}
						self.macro_stack.extend(replacement.iter().rev());
					}
					return Ok(());
				}
			}

			x => panic!("Preprocessor directive {:?} not implemented", x)
		}

		return Err(
			DiagBuilder2::fatal(format!("Unknown compiler directive '`{}'", dir_name))
			.span(span)
		);

		// panic!("Unknown compiler directive '`{}'", dir);
		// Ok(())
	}

	fn open_include(&mut self, filename: &str, current_file: &str) -> Option<Source> {
		// println!("Resolving include '{}' from '{}'", filename, current_file);
		let first = [Path::new(current_file)
			.parent()
			.expect("current file path must have a valid parent")];
		let prefices = first.iter().chain(self.include_paths.iter());
		let sm = get_source_manager();
		for prefix in prefices {
			let mut buf = prefix.to_path_buf();
			buf.push(filename);
			// println!("  trying {}", buf.to_str().unwrap());
			let src = sm.open(buf.to_str().unwrap());
			if src.is_some() {
				return src;
			}
		}
		return None;
	}

	/// Check whether we are inside a disabled define conditional. That is,
	/// whether a preceeding `ifdef, `ifndef, `else, or `elsif directive have
	/// disabled the subsequent code.
	fn is_inactive(&self) -> bool {
		match self.defcond_stack.last() {
			Some(&Defcond::Enabled) | None => false,
			_ => true,
		}
	}
}

impl<'a> Iterator for Preprocessor<'a> {
	type Item = DiagResult2<TokenAndSpan>;

	fn next(&mut self) -> Option<DiagResult2<TokenAndSpan>> {
		// In case this is the first call to next(), the token has not been
		// populated yet. In this case we need to artificially bump the lexer.
		if self.token.is_none() {
			self.bump();
		}
		loop {
			// This is the main loop of the lexer. Upon each iteration the next
			// token is inspected and the lexer decides whether to emit it or
			// not. If no token was emitted (e.g. because it was a preprocessor
			// directive or we're inside an inactive `ifdef block), the loop
			// continues with the next token.
			match self.token {
				Some((Symbol('`'), sp_backtick)) => {
					self.bump(); // consume the backtick
					match self.token {
						Some((Text, sp)) => {
							// We arrive here if the sequence a backtick
							// followed by text was encountered. In this case we
							// call upon the handle_directive function to
							// perform the necessary actions.
							let dir_span = Span::union(sp_backtick, sp);
							match self.handle_directive(sp.extract(), dir_span) {
								Err(x) => return Some(Err(x)),
								_ => ()
							}

							// It is important that the lexer is bumped here,
							// after handling the directive. This makes sure
							// that if an include was handled, the next token
							// after the directive actually comes from that
							// file.
							self.bump();
							continue;
						}
						Some((Symbol('`'), sp)) => {
							return Some(Err(
								DiagBuilder2::fatal("Preprocessor concatenation '``' used outside of `define")
								.span(Span::union(sp_backtick, sp))
							));
						}
						_ => {
							return Some(Err(
								DiagBuilder2::fatal("Expected compiler directive after '`'")
								.span(sp_backtick)
							));
						}
					}
				}
				_ => {
					// All tokens other than preprocessor directives are
					// emitted, unless we're currently inside a disabled define
					// conditional.
					if self.is_inactive() {
						self.bump();
					} else {
						let tkn = self.token.map(|x| Ok(x));
						self.bump();
						return tkn;
					}
				}
			}
		}
	}
}



struct Stream<'a> {
	source: Source,
	iter: Cat<'a>,
}

/// The different compiler directives recognized by the preprocessor.
#[derive(Debug, Clone, Copy)]
enum Directive {
	Include,
	Define,
	Undef,
	Undefineall,
	Ifdef,
	Ifndef,
	Else,
	Elsif,
	Endif,
	Unknown,
}

thread_local!(static DIRECTIVES_TABLE: HashMap<&'static str, Directive> = {
	use self::Directive::*;
	let mut table = HashMap::new();
	table.insert("include", Include);
	table.insert("define", Define);
	table.insert("undef", Undef);
	table.insert("undefineall", Undefineall);
	table.insert("ifdef", Ifdef);
	table.insert("ifndef", Ifndef);
	table.insert("else", Else);
	table.insert("elsif", Elsif);
	table.insert("endif", Endif);
	table
});



#[derive(Debug)]
struct Macro {
	name: String,
	span: Span,
	args: Vec<MacroArg>,
	body: Vec<TokenAndSpan>,
}

impl Macro {
	fn new(name: String, span: Span) -> Macro {
		Macro {
			name: name,
			span: span,
			args: Vec::new(),
			body: Vec::new(),
		}
	}
}

#[derive(Debug)]
struct MacroArg {
	name: String,
	span: Span,
}

impl MacroArg {
	fn new(name: String, span: Span) -> MacroArg {
		MacroArg {
			name: name,
			span: span,
		}
	}
}

enum Defcond {
	Done,
	Enabled,
	Disabled,
}



#[cfg(test)]
mod tests {
	use super::*;
	use source::*;

	#[test]
	fn include_and_define() {
		let sm = get_source_manager();
		sm.add("other.sv", "/* World */\n`define foo 42\n");
		sm.add("test.sv", "// Hello\n`include \"other.sv\"\n`foo something\n");
		let mut pp = Preprocessor::new(sm.open("test.sv").unwrap(), &[]);
		let actual: String = pp.map(|x| x.unwrap().1.extract()).collect();
		let expected = "// Hello\n/* World */\n\n42 something\n";
		assert_eq!(actual, expected);
	}

	#[test]
	#[should_panic(expected = "Unknown compiler directive")]
	fn conditional_define() {
		let sm = get_source_manager();
		let source = sm.add("test.sv", "`ifdef FOO\n`define BAR\n`endif\n`BAR");
		let mut pp = Preprocessor::new(source, &[]);
		while let Some(tkn) = pp.next() {
			tkn.unwrap();
		}
	}

	#[test]
	fn macro_args() {
		let sm = get_source_manager();
		let source = sm.add("test.sv", "`define foo(x,y) {x + y _bar}\n`foo(12, foo)\n");
		let mut pp = Preprocessor::new(source, &[]);
		let actual: String = pp.map(|x| x.unwrap().1.extract()).collect();
		let expected = "{12 +  foo _bar}\n";
		assert_eq!(actual, expected);
	}

	/// Verify that macros that take no arguments but have parantheses around
	/// their body parse properly.
	#[test]
	fn macro_noargs_parentheses() {
		let sm = get_source_manager();
		let source = sm.add("test.sv", "`define FOO 4\n`define BAR (`FOO+$clog2(2))\n`BAR");
		let mut pp = Preprocessor::new(source, &[]);
		let actual: String = pp.map(|x| x.unwrap().1.extract()).collect();
		let expected = "(4+$clog2(2))";
		assert_eq!(actual, expected);
	}
}
