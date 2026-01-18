//! Role-Based Access Control (RBAC) for Ralph.
//!
//! This module provides a flexible RBAC system with:
//! - Built-in roles: Admin, Developer, Viewer
//! - Custom role creation
//! - Permission checking on resources
//!
//! # Example
//!
//! ```rust
//! use ralph::enterprise::rbac::{Role, Permission, PermissionCheck};
//!
//! // Built-in roles have predefined permissions
//! let admin = Role::Admin;
//! assert!(admin.has_permission(&Permission::Write));
//! assert!(admin.has_permission(&Permission::Admin));
//!
//! let developer = Role::Developer;
//! assert!(developer.has_permission(&Permission::Write));
//! assert!(!developer.has_permission(&Permission::Admin));
//!
//! let viewer = Role::Viewer;
//! assert!(viewer.has_permission(&Permission::Read));
//! assert!(!viewer.has_permission(&Permission::Write));
//!
//! // Create custom roles
//! use ralph::enterprise::rbac::RoleBuilder;
//!
//! let custom = RoleBuilder::new("reviewer")
//!     .with_permission(Permission::Read)
//!     .with_permission(Permission::Review)
//!     .build();
//! assert!(custom.has_permission(&Permission::Read));
//! assert!(custom.has_permission(&Permission::Review));
//! assert!(!custom.has_permission(&Permission::Write));
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

/// Permissions that can be granted to roles.
///
/// # Example
///
/// ```rust
/// use ralph::enterprise::rbac::Permission;
///
/// let perm = Permission::Read;
/// assert_eq!(perm.to_string(), "read");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// Read access to resources
    Read,
    /// Write/modify access to resources
    Write,
    /// Delete access to resources
    Delete,
    /// Execute campaigns and loops
    Execute,
    /// Review and approve changes
    Review,
    /// Administrative access (user management, settings)
    Admin,
    /// Manage quality gates and thresholds
    ManageQuality,
    /// View audit logs
    ViewAudit,
    /// Export data
    Export,
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Permission::Read => write!(f, "read"),
            Permission::Write => write!(f, "write"),
            Permission::Delete => write!(f, "delete"),
            Permission::Execute => write!(f, "execute"),
            Permission::Review => write!(f, "review"),
            Permission::Admin => write!(f, "admin"),
            Permission::ManageQuality => write!(f, "manage_quality"),
            Permission::ViewAudit => write!(f, "view_audit"),
            Permission::Export => write!(f, "export"),
        }
    }
}

impl std::str::FromStr for Permission {
    type Err = ParsePermissionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "read" => Ok(Permission::Read),
            "write" => Ok(Permission::Write),
            "delete" => Ok(Permission::Delete),
            "execute" => Ok(Permission::Execute),
            "review" => Ok(Permission::Review),
            "admin" => Ok(Permission::Admin),
            "manage_quality" => Ok(Permission::ManageQuality),
            "view_audit" => Ok(Permission::ViewAudit),
            "export" => Ok(Permission::Export),
            _ => Err(ParsePermissionError(s.to_string())),
        }
    }
}

/// Error returned when parsing an invalid permission string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsePermissionError(pub String);

impl fmt::Display for ParsePermissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid permission: {}", self.0)
    }
}

impl std::error::Error for ParsePermissionError {}

/// A set of permissions that can be assigned to a role.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RolePermissions {
    permissions: HashSet<Permission>,
}

impl RolePermissions {
    /// Create an empty permission set.
    pub fn new() -> Self {
        Self {
            permissions: HashSet::new(),
        }
    }

    /// Create a permission set from a collection of permissions.
    pub fn from_permissions<I: IntoIterator<Item = Permission>>(iter: I) -> Self {
        Self {
            permissions: iter.into_iter().collect(),
        }
    }

    /// Add a permission to the set.
    pub fn add(&mut self, permission: Permission) {
        self.permissions.insert(permission);
    }

    /// Remove a permission from the set.
    pub fn remove(&mut self, permission: &Permission) -> bool {
        self.permissions.remove(permission)
    }

    /// Check if the set contains a permission.
    pub fn contains(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }

    /// Get all permissions in the set.
    pub fn iter(&self) -> impl Iterator<Item = &Permission> {
        self.permissions.iter()
    }

    /// Get the number of permissions.
    pub fn len(&self) -> usize {
        self.permissions.len()
    }

    /// Check if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.permissions.is_empty()
    }

    /// Merge another permission set into this one.
    pub fn merge(&mut self, other: &RolePermissions) {
        for perm in &other.permissions {
            self.permissions.insert(*perm);
        }
    }
}

/// Built-in roles with predefined permissions.
///
/// # Roles
///
/// - **Admin**: Full access to all features
/// - **Developer**: Read, write, execute, and export access
/// - **Viewer**: Read-only access (default)
///
/// # Example
///
/// ```rust
/// use ralph::enterprise::rbac::{Role, Permission, PermissionCheck};
///
/// let admin = Role::Admin;
/// assert!(admin.has_permission(&Permission::Admin));
///
/// let viewer = Role::Viewer;
/// assert!(viewer.has_permission(&Permission::Read));
/// assert!(!viewer.has_permission(&Permission::Write));
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Role {
    /// Full administrative access
    Admin,
    /// Standard development access
    Developer,
    /// Read-only access (default)
    #[default]
    Viewer,
    /// Custom role with specific permissions
    Custom(CustomRole),
}

impl Role {
    /// Get the name of this role.
    pub fn name(&self) -> &str {
        match self {
            Role::Admin => "admin",
            Role::Developer => "developer",
            Role::Viewer => "viewer",
            Role::Custom(custom) => &custom.name,
        }
    }

    /// Get the permissions for this role.
    pub fn permissions(&self) -> RolePermissions {
        match self {
            Role::Admin => Self::admin_permissions(),
            Role::Developer => Self::developer_permissions(),
            Role::Viewer => Self::viewer_permissions(),
            Role::Custom(custom) => custom.permissions.clone(),
        }
    }

    fn admin_permissions() -> RolePermissions {
        RolePermissions::from_permissions([
            Permission::Read,
            Permission::Write,
            Permission::Delete,
            Permission::Execute,
            Permission::Review,
            Permission::Admin,
            Permission::ManageQuality,
            Permission::ViewAudit,
            Permission::Export,
        ])
    }

    fn developer_permissions() -> RolePermissions {
        RolePermissions::from_permissions([
            Permission::Read,
            Permission::Write,
            Permission::Execute,
            Permission::Export,
        ])
    }

    fn viewer_permissions() -> RolePermissions {
        RolePermissions::from_permissions([Permission::Read])
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl std::str::FromStr for Role {
    type Err = ParseRoleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "admin" => Ok(Role::Admin),
            "developer" => Ok(Role::Developer),
            "viewer" => Ok(Role::Viewer),
            _ => Err(ParseRoleError(s.to_string())),
        }
    }
}

/// Error returned when parsing an invalid role string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseRoleError(pub String);

impl fmt::Display for ParseRoleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid role: {} (expected: admin, developer, viewer)", self.0)
    }
}

impl std::error::Error for ParseRoleError {}

/// A custom role with a specific set of permissions.
///
/// Custom roles are created using the [`RoleBuilder`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomRole {
    /// Name of the custom role
    pub name: String,
    /// Description of what this role is for
    pub description: Option<String>,
    /// Permissions granted to this role
    pub permissions: RolePermissions,
}

impl CustomRole {
    /// Create a new custom role.
    ///
    /// Use [`RoleBuilder`] for a more ergonomic API.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            permissions: RolePermissions::new(),
        }
    }
}

/// Builder for creating custom roles.
///
/// # Example
///
/// ```rust
/// use ralph::enterprise::rbac::{RoleBuilder, Permission, PermissionCheck};
///
/// let role = RoleBuilder::new("qa_engineer")
///     .description("Quality assurance engineer with review access")
///     .with_permission(Permission::Read)
///     .with_permission(Permission::Review)
///     .with_permission(Permission::ViewAudit)
///     .build();
///
/// assert_eq!(role.name(), "qa_engineer");
/// assert!(role.has_permission(&Permission::Review));
/// assert!(!role.has_permission(&Permission::Write));
/// ```
#[derive(Debug)]
pub struct RoleBuilder {
    name: String,
    description: Option<String>,
    permissions: RolePermissions,
}

impl RoleBuilder {
    /// Create a new role builder with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            permissions: RolePermissions::new(),
        }
    }

    /// Set the description for this role.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Add a permission to this role.
    pub fn with_permission(mut self, permission: Permission) -> Self {
        self.permissions.add(permission);
        self
    }

    /// Add multiple permissions to this role.
    pub fn with_permissions<I: IntoIterator<Item = Permission>>(mut self, permissions: I) -> Self {
        for perm in permissions {
            self.permissions.add(perm);
        }
        self
    }

    /// Build the custom role.
    pub fn build(self) -> Role {
        Role::Custom(CustomRole {
            name: self.name,
            description: self.description,
            permissions: self.permissions,
        })
    }
}

/// Trait for checking permissions.
///
/// Implemented by [`Role`] to check if a role has a specific permission.
pub trait PermissionCheck {
    /// Check if this entity has the given permission.
    fn has_permission(&self, permission: &Permission) -> bool;

    /// Check if this entity has all of the given permissions.
    fn has_all_permissions<'a, I>(&self, permissions: I) -> bool
    where
        I: IntoIterator<Item = &'a Permission>,
    {
        permissions.into_iter().all(|p| self.has_permission(p))
    }

    /// Check if this entity has any of the given permissions.
    fn has_any_permission<'a, I>(&self, permissions: I) -> bool
    where
        I: IntoIterator<Item = &'a Permission>,
    {
        permissions.into_iter().any(|p| self.has_permission(p))
    }
}

impl PermissionCheck for Role {
    fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions().contains(permission)
    }
}

impl PermissionCheck for RolePermissions {
    fn has_permission(&self, permission: &Permission) -> bool {
        self.contains(permission)
    }
}

/// A resource that can have permissions checked against it.
///
/// This is used to define what operations are required for accessing
/// different types of resources.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Resource {
    /// Project resources
    Project,
    /// Campaign resources
    Campaign,
    /// Quality gate resources
    QualityGate,
    /// User/team management
    UserManagement,
    /// Audit logs
    AuditLog,
    /// System settings
    Settings,
}

impl Resource {
    /// Get the permission required to read this resource.
    pub fn read_permission(&self) -> Permission {
        match self {
            Resource::AuditLog => Permission::ViewAudit,
            _ => Permission::Read,
        }
    }

    /// Get the permission required to write/modify this resource.
    pub fn write_permission(&self) -> Permission {
        match self {
            Resource::QualityGate => Permission::ManageQuality,
            Resource::UserManagement | Resource::Settings => Permission::Admin,
            _ => Permission::Write,
        }
    }

    /// Get the permission required to delete this resource.
    pub fn delete_permission(&self) -> Permission {
        match self {
            Resource::UserManagement | Resource::Settings => Permission::Admin,
            _ => Permission::Delete,
        }
    }
}

/// Check if a role can perform an action on a resource.
///
/// # Example
///
/// ```rust
/// use ralph::enterprise::rbac::{Role, Resource, can_access};
///
/// let developer = Role::Developer;
/// assert!(can_access(&developer, &Resource::Project, "read"));
/// assert!(can_access(&developer, &Resource::Project, "write"));
/// assert!(!can_access(&developer, &Resource::UserManagement, "write"));
///
/// let viewer = Role::Viewer;
/// assert!(can_access(&viewer, &Resource::Project, "read"));
/// assert!(!can_access(&viewer, &Resource::Project, "write"));
/// ```
pub fn can_access(role: &Role, resource: &Resource, action: &str) -> bool {
    let required_permission = match action.to_lowercase().as_str() {
        "read" => resource.read_permission(),
        "write" | "modify" | "update" => resource.write_permission(),
        "delete" | "remove" => resource.delete_permission(),
        "execute" | "run" => Permission::Execute,
        "review" | "approve" => Permission::Review,
        "export" => Permission::Export,
        _ => return false,
    };

    role.has_permission(&required_permission)
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Permission Tests
    // =========================================================================

    #[test]
    fn test_permission_display() {
        assert_eq!(Permission::Read.to_string(), "read");
        assert_eq!(Permission::Write.to_string(), "write");
        assert_eq!(Permission::Delete.to_string(), "delete");
        assert_eq!(Permission::Execute.to_string(), "execute");
        assert_eq!(Permission::Review.to_string(), "review");
        assert_eq!(Permission::Admin.to_string(), "admin");
        assert_eq!(Permission::ManageQuality.to_string(), "manage_quality");
        assert_eq!(Permission::ViewAudit.to_string(), "view_audit");
        assert_eq!(Permission::Export.to_string(), "export");
    }

    #[test]
    fn test_permission_from_str() {
        assert_eq!("read".parse::<Permission>().unwrap(), Permission::Read);
        assert_eq!("WRITE".parse::<Permission>().unwrap(), Permission::Write);
        assert_eq!("Admin".parse::<Permission>().unwrap(), Permission::Admin);
        assert!("invalid".parse::<Permission>().is_err());
    }

    #[test]
    fn test_permission_equality() {
        assert_eq!(Permission::Read, Permission::Read);
        assert_ne!(Permission::Read, Permission::Write);
    }

    #[test]
    fn test_permission_hash() {
        let mut set = HashSet::new();
        set.insert(Permission::Read);
        set.insert(Permission::Write);
        set.insert(Permission::Read); // duplicate

        assert_eq!(set.len(), 2);
        assert!(set.contains(&Permission::Read));
        assert!(set.contains(&Permission::Write));
    }

    // =========================================================================
    // RolePermissions Tests
    // =========================================================================

    #[test]
    fn test_role_permissions_new() {
        let perms = RolePermissions::new();
        assert!(perms.is_empty());
        assert_eq!(perms.len(), 0);
    }

    #[test]
    fn test_role_permissions_add_and_contains() {
        let mut perms = RolePermissions::new();
        perms.add(Permission::Read);
        perms.add(Permission::Write);

        assert!(perms.contains(&Permission::Read));
        assert!(perms.contains(&Permission::Write));
        assert!(!perms.contains(&Permission::Admin));
        assert_eq!(perms.len(), 2);
    }

    #[test]
    fn test_role_permissions_remove() {
        let mut perms = RolePermissions::new();
        perms.add(Permission::Read);
        perms.add(Permission::Write);

        assert!(perms.remove(&Permission::Read));
        assert!(!perms.contains(&Permission::Read));
        assert!(perms.contains(&Permission::Write));

        // Removing non-existent permission returns false
        assert!(!perms.remove(&Permission::Admin));
    }

    #[test]
    fn test_role_permissions_from_iter() {
        let perms = RolePermissions::from_permissions([
            Permission::Read,
            Permission::Write,
            Permission::Execute,
        ]);

        assert_eq!(perms.len(), 3);
        assert!(perms.contains(&Permission::Read));
        assert!(perms.contains(&Permission::Write));
        assert!(perms.contains(&Permission::Execute));
    }

    #[test]
    fn test_role_permissions_merge() {
        let mut perms1 = RolePermissions::from_permissions([Permission::Read]);
        let perms2 = RolePermissions::from_permissions([Permission::Write, Permission::Execute]);

        perms1.merge(&perms2);

        assert_eq!(perms1.len(), 3);
        assert!(perms1.contains(&Permission::Read));
        assert!(perms1.contains(&Permission::Write));
        assert!(perms1.contains(&Permission::Execute));
    }

    #[test]
    fn test_role_permissions_iter() {
        let perms = RolePermissions::from_permissions([Permission::Read, Permission::Write]);
        let collected: HashSet<_> = perms.iter().copied().collect();

        assert!(collected.contains(&Permission::Read));
        assert!(collected.contains(&Permission::Write));
    }

    // =========================================================================
    // Role Tests - Built-in Roles
    // =========================================================================

    #[test]
    fn test_role_admin_permissions() {
        let admin = Role::Admin;

        assert_eq!(admin.name(), "admin");
        assert!(admin.has_permission(&Permission::Read));
        assert!(admin.has_permission(&Permission::Write));
        assert!(admin.has_permission(&Permission::Delete));
        assert!(admin.has_permission(&Permission::Execute));
        assert!(admin.has_permission(&Permission::Review));
        assert!(admin.has_permission(&Permission::Admin));
        assert!(admin.has_permission(&Permission::ManageQuality));
        assert!(admin.has_permission(&Permission::ViewAudit));
        assert!(admin.has_permission(&Permission::Export));
    }

    #[test]
    fn test_role_developer_permissions() {
        let developer = Role::Developer;

        assert_eq!(developer.name(), "developer");
        assert!(developer.has_permission(&Permission::Read));
        assert!(developer.has_permission(&Permission::Write));
        assert!(developer.has_permission(&Permission::Execute));
        assert!(developer.has_permission(&Permission::Export));

        // Developer should NOT have these permissions
        assert!(!developer.has_permission(&Permission::Delete));
        assert!(!developer.has_permission(&Permission::Review));
        assert!(!developer.has_permission(&Permission::Admin));
        assert!(!developer.has_permission(&Permission::ManageQuality));
        assert!(!developer.has_permission(&Permission::ViewAudit));
    }

    #[test]
    fn test_role_viewer_permissions() {
        let viewer = Role::Viewer;

        assert_eq!(viewer.name(), "viewer");
        assert!(viewer.has_permission(&Permission::Read));

        // Viewer should NOT have any other permissions
        assert!(!viewer.has_permission(&Permission::Write));
        assert!(!viewer.has_permission(&Permission::Delete));
        assert!(!viewer.has_permission(&Permission::Execute));
        assert!(!viewer.has_permission(&Permission::Review));
        assert!(!viewer.has_permission(&Permission::Admin));
        assert!(!viewer.has_permission(&Permission::ManageQuality));
        assert!(!viewer.has_permission(&Permission::ViewAudit));
        assert!(!viewer.has_permission(&Permission::Export));
    }

    #[test]
    fn test_role_default() {
        let role = Role::default();
        assert_eq!(role.name(), "viewer");
    }

    #[test]
    fn test_role_display() {
        assert_eq!(Role::Admin.to_string(), "admin");
        assert_eq!(Role::Developer.to_string(), "developer");
        assert_eq!(Role::Viewer.to_string(), "viewer");
    }

    #[test]
    fn test_role_from_str() {
        assert!(matches!("admin".parse::<Role>().unwrap(), Role::Admin));
        assert!(matches!("DEVELOPER".parse::<Role>().unwrap(), Role::Developer));
        assert!(matches!("Viewer".parse::<Role>().unwrap(), Role::Viewer));
        assert!("custom".parse::<Role>().is_err());
    }

    // =========================================================================
    // Custom Role Tests
    // =========================================================================

    #[test]
    fn test_custom_role_new() {
        let custom = CustomRole::new("test_role");
        assert_eq!(custom.name, "test_role");
        assert!(custom.description.is_none());
        assert!(custom.permissions.is_empty());
    }

    #[test]
    fn test_role_builder_basic() {
        let role = RoleBuilder::new("qa_engineer").build();

        assert_eq!(role.name(), "qa_engineer");
        assert!(role.permissions().is_empty());
    }

    #[test]
    fn test_role_builder_with_description() {
        let role = RoleBuilder::new("qa_engineer")
            .description("Quality assurance engineer")
            .build();

        if let Role::Custom(custom) = role {
            assert_eq!(custom.description, Some("Quality assurance engineer".to_string()));
        } else {
            panic!("Expected custom role");
        }
    }

    #[test]
    fn test_role_builder_with_permissions() {
        let role = RoleBuilder::new("reviewer")
            .with_permission(Permission::Read)
            .with_permission(Permission::Review)
            .with_permission(Permission::ViewAudit)
            .build();

        assert_eq!(role.name(), "reviewer");
        assert!(role.has_permission(&Permission::Read));
        assert!(role.has_permission(&Permission::Review));
        assert!(role.has_permission(&Permission::ViewAudit));
        assert!(!role.has_permission(&Permission::Write));
        assert!(!role.has_permission(&Permission::Admin));
    }

    #[test]
    fn test_role_builder_with_multiple_permissions() {
        let role = RoleBuilder::new("power_user")
            .with_permissions([
                Permission::Read,
                Permission::Write,
                Permission::Execute,
                Permission::Export,
            ])
            .build();

        assert!(role.has_permission(&Permission::Read));
        assert!(role.has_permission(&Permission::Write));
        assert!(role.has_permission(&Permission::Execute));
        assert!(role.has_permission(&Permission::Export));
        assert!(!role.has_permission(&Permission::Admin));
    }

    #[test]
    fn test_custom_role_display() {
        let role = RoleBuilder::new("my_custom_role").build();
        assert_eq!(role.to_string(), "my_custom_role");
    }

    // =========================================================================
    // PermissionCheck Trait Tests
    // =========================================================================

    #[test]
    fn test_has_all_permissions() {
        let admin = Role::Admin;
        let viewer = Role::Viewer;

        assert!(admin.has_all_permissions(&[Permission::Read, Permission::Write, Permission::Admin]));
        assert!(!viewer.has_all_permissions(&[Permission::Read, Permission::Write]));
    }

    #[test]
    fn test_has_any_permission() {
        let viewer = Role::Viewer;
        let developer = Role::Developer;

        assert!(viewer.has_any_permission(&[Permission::Read, Permission::Admin]));
        assert!(!viewer.has_any_permission(&[Permission::Write, Permission::Admin]));
        assert!(developer.has_any_permission(&[Permission::Admin, Permission::Execute]));
    }

    // =========================================================================
    // Resource Tests
    // =========================================================================

    #[test]
    fn test_resource_read_permission() {
        assert_eq!(Resource::Project.read_permission(), Permission::Read);
        assert_eq!(Resource::Campaign.read_permission(), Permission::Read);
        assert_eq!(Resource::AuditLog.read_permission(), Permission::ViewAudit);
    }

    #[test]
    fn test_resource_write_permission() {
        assert_eq!(Resource::Project.write_permission(), Permission::Write);
        assert_eq!(Resource::QualityGate.write_permission(), Permission::ManageQuality);
        assert_eq!(Resource::UserManagement.write_permission(), Permission::Admin);
        assert_eq!(Resource::Settings.write_permission(), Permission::Admin);
    }

    #[test]
    fn test_resource_delete_permission() {
        assert_eq!(Resource::Project.delete_permission(), Permission::Delete);
        assert_eq!(Resource::UserManagement.delete_permission(), Permission::Admin);
    }

    // =========================================================================
    // can_access Function Tests
    // =========================================================================

    #[test]
    fn test_can_access_admin() {
        let admin = Role::Admin;

        assert!(can_access(&admin, &Resource::Project, "read"));
        assert!(can_access(&admin, &Resource::Project, "write"));
        assert!(can_access(&admin, &Resource::Project, "delete"));
        assert!(can_access(&admin, &Resource::UserManagement, "write"));
        assert!(can_access(&admin, &Resource::AuditLog, "read"));
        assert!(can_access(&admin, &Resource::QualityGate, "write"));
    }

    #[test]
    fn test_can_access_developer() {
        let developer = Role::Developer;

        assert!(can_access(&developer, &Resource::Project, "read"));
        assert!(can_access(&developer, &Resource::Project, "write"));
        assert!(can_access(&developer, &Resource::Campaign, "execute"));
        assert!(can_access(&developer, &Resource::Project, "export"));

        // Developer should NOT have admin access
        assert!(!can_access(&developer, &Resource::UserManagement, "write"));
        assert!(!can_access(&developer, &Resource::AuditLog, "read"));
        assert!(!can_access(&developer, &Resource::Project, "delete"));
    }

    #[test]
    fn test_can_access_viewer() {
        let viewer = Role::Viewer;

        assert!(can_access(&viewer, &Resource::Project, "read"));
        assert!(can_access(&viewer, &Resource::Campaign, "read"));

        // Viewer should only have read access
        assert!(!can_access(&viewer, &Resource::Project, "write"));
        assert!(!can_access(&viewer, &Resource::Project, "delete"));
        assert!(!can_access(&viewer, &Resource::Campaign, "execute"));
        assert!(!can_access(&viewer, &Resource::Project, "export"));
    }

    #[test]
    fn test_can_access_custom_role() {
        let custom = RoleBuilder::new("reviewer")
            .with_permission(Permission::Read)
            .with_permission(Permission::Review)
            .build();

        assert!(can_access(&custom, &Resource::Project, "read"));
        assert!(can_access(&custom, &Resource::Campaign, "review"));
        assert!(!can_access(&custom, &Resource::Project, "write"));
        assert!(!can_access(&custom, &Resource::Project, "delete"));
    }

    #[test]
    fn test_can_access_invalid_action() {
        let admin = Role::Admin;
        assert!(!can_access(&admin, &Resource::Project, "invalid_action"));
    }

    #[test]
    fn test_can_access_action_aliases() {
        let developer = Role::Developer;

        // Test action aliases
        assert!(can_access(&developer, &Resource::Project, "modify"));
        assert!(can_access(&developer, &Resource::Project, "update"));
        assert!(can_access(&developer, &Resource::Campaign, "run"));
    }

    // =========================================================================
    // Serialization Tests
    // =========================================================================

    #[test]
    fn test_permission_serde() {
        let perm = Permission::ManageQuality;
        let json = serde_json::to_string(&perm).unwrap();
        assert_eq!(json, "\"manage_quality\"");

        let parsed: Permission = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Permission::ManageQuality);
    }

    #[test]
    fn test_role_serde_builtin() {
        let admin = Role::Admin;
        let json = serde_json::to_string(&admin).unwrap();

        let parsed: Role = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Role::Admin));
    }

    #[test]
    fn test_role_serde_custom() {
        let custom = RoleBuilder::new("test")
            .description("Test role")
            .with_permission(Permission::Read)
            .with_permission(Permission::Write)
            .build();

        let json = serde_json::to_string(&custom).unwrap();
        let parsed: Role = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name(), "test");
        assert!(parsed.has_permission(&Permission::Read));
        assert!(parsed.has_permission(&Permission::Write));
    }
}
