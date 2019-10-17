use crate::error::{ApiError, QaulError};
use japi::{Attributes, ResourceObject, Relationships, Relationship, OptionalVec, Links, 
    Link, Identifier};
use serde_derive::{Serialize, Deserialize};
use libqaul::UserAuth;
use super::from_identity;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Grant { 
    secret: String,
}

impl Attributes for Grant {
    fn kind() -> String { "grant".into() }
}

impl Grant {
    fn from_user_auth(ua: UserAuth) -> Result<ResourceObject<Grant>, ApiError> {
        let (id, grant) = ua.trusted().map_err(|e| QaulError::from(e))?;
        let mut g = ResourceObject::<Grant>::new(grant.clone(), None);

        let mut relationships = Relationships::new();
        relationships.insert("user".into(), Relationship {
            data: OptionalVec::One(Some(Identifier::new(from_identity(&id), "user".into()))),
            ..Default::default()
        });
        g.relationships = Some(relationships);

        let mut links = Links::new();
        links.insert("self".into(), Link::Url(format!("/api/grants/{}", grant)));
        g.links = Some(links);

        Ok(g)
    }
 }