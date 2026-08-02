# Extensions and deployment security

Native Rust extensions implement version-1 traits for tasks, tools, model adapters, policies, event stores, or graders and register at compile time. API descriptors fail before registration when versions differ. Python plugins remain process-isolated and now exchange versioned envelopes; protocol mismatch fails closed.

Deployment policy JSON can allow effects/tools, constrain network hosts and filesystem roots, and require approval around external writes. `--summary` makes external writes and approval boundaries visible before execution. Trace and durable history redaction occurs before persistence.

Trust boundaries are explicit: native Rust shares the host process and is fully trusted; Python has process isolation but retains OS permissions granted to the host; a future WASI boundary is deferred until the native contracts stabilize and real untrusted-extension requirements emerge.
