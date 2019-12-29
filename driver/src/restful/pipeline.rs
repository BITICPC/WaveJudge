//! This module defines abstractions of the request pipeline for the RESTful client.
//!

use reqwest::{
    RequestBuilder,
    Response,
};

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        ReqwestError(::reqwest::Error);
    }

    errors {
        MiddlewareError(message: String) {
            description("middleware error")
            display("middleware error: {}", message)
        }
    }
}

/// Trait for a middleware in the pipeline.
pub trait Middleware : Sync + Send {
    /// Execute the middleware logic on the given `PipelineContext`.
    fn handle(&self, context: PipelineContext<'_>) -> Result<Response>;
}

/// Represent a HTTP request pipeline.
pub struct Pipeline {
    /// The middlewares for handling each input HTTP request.
    middlewares: Vec<Box<dyn Middleware>>,
}

impl Pipeline {
    /// Create a new `Pipeline` instance.
    pub fn new() -> Self {
        Pipeline {
            middlewares: Vec::new(),
        }
    }

    /// Add the given middleware to the middleware pipeline.
    pub fn add_middleware(&mut self, middleware: Box<dyn Middleware>) {
        self.middlewares.push(middleware);
    }

    /// Execute the request, using the given HTTP client.
    pub fn execute(&self, req: RequestBuilder) -> Result<Response> {
        let context = PipelineContext::new(self, req);
        context.invoke_next()
    }
}

/// Provide a context for the execution of a pipeline.
pub struct PipelineContext<'a> {
    /// Reference to the pipeline object.
    pipeline: &'a Pipeline,

    /// The request.
    request: Option<RequestBuilder>,

    /// Index of the next middleware to be invoked.
    next_index: usize,
}

impl<'a> PipelineContext<'a> {
    /// Create a new `PipelineContext` object.
    fn new(pipeline: &'a Pipeline, request: RequestBuilder) -> Self {
        PipelineContext {
            pipeline,
            request: Some(request),
            next_index: 0,
        }
    }

    /// Transfer the ownership of the underlying request to the caller.
    fn take_request(&mut self) -> RequestBuilder {
        self.request.take().unwrap()
    }

    /// Try clone this `PipelineContext` object.
    pub fn try_clone(&self) -> Option<Self> {
        let clone_request = match self.request {
            Some(ref r) => match r.try_clone() {
                Some(r) => Some(r),
                None => return None
            },
            None => None
        };

        let cloned = PipelineContext {
            pipeline: self.pipeline,
            request: clone_request,
            next_index: self.next_index,
        };
        Some(cloned)
    }

    /// Execute the given callback on the underlying request builder.
    pub fn map_request<F>(&mut self, mapper: F)
        where F: FnOnce(RequestBuilder) -> RequestBuilder {
        self.request = self.request.take().map(mapper)
    }

    /// Invoke the next middleware in the pipeline.
    pub fn invoke_next(mut self) -> Result<Response> {
        if self.next_index == self.pipeline.middlewares.len() {
            // All middleware has been invoked. Execute the request.
            let response = self.take_request().send()?;
            return Ok(response);
        }

        let middleware = self.pipeline.middlewares.get(self.next_index).unwrap();
        self.next_index += 1;

        middleware.handle(self)
    }
}
