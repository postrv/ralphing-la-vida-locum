//! Enterprise features for Ralph.
//!
//! This module provides enterprise-grade features including:
//! - Role-Based Access Control (RBAC)
//! - Audit logging
//! - Team management
//!
//! These features are designed to be extended by ralph-cloud for
//! full enterprise functionality.

pub mod rbac;

// Re-export main types
pub use rbac::{Permission, PermissionCheck, Role, RoleBuilder, RolePermissions};
