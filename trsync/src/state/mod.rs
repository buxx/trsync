use std::path::PathBuf;

use anyhow::{Context, Result as AnyhowResult};

use trsync_core::{
    content::Content,
    error::StateError,
    instance::{ContentFileName, ContentId, DiskTimestamp, RevisionId},
};

use crate::path::ContentPath;

use self::modification::StateModification;

pub mod disk;
pub mod memory;
pub mod modification;

pub trait State {
    fn known(&self, id: ContentId) -> AnyhowResult<bool>;
    fn get(&self, id: ContentId) -> AnyhowResult<Option<Content>>;
    fn content_id_for_path(&self, path: PathBuf) -> AnyhowResult<Option<ContentId>>;
    // Path must be build on demand because parent hierarchy can change
    fn path(&self, id: ContentId) -> AnyhowResult<ContentPath, StateError>;
    // TODO : Iter
    // pub trait Trait {
    //     type Iter<'a>: Iterator<Item = &'a Content> + 'a
    //     where
    //         Self: 'a;
    //     fn contents(&self) -> AnyhowResult<Self::Iter<'_>>;
    // }

    // impl Trait for Map {
    //     type Iter<'a> = Values<'a, ContentId, Content>;
    //     fn contents(&self) -> AnyhowResult<Self::Iter<'_>> {
    //         Ok(self.contents.values())
    //     }
    // }
    /// Return iterable of `&Contents` ordered by `ContentType::Folder` first
    fn contents(&self) -> AnyhowResult<Vec<Content>>;
    fn direct_children_ids(&self, content_id: ContentId) -> AnyhowResult<Vec<ContentId>>;
    fn forgot(&mut self, content_id: ContentId) -> AnyhowResult<()>;
    fn add(
        &mut self,
        content: Content,
        relative_path: PathBuf,
        timestamp: DiskTimestamp,
    ) -> Result<(), StateError>;
    fn update(
        &mut self,
        content_id: ContentId,
        file_name: ContentFileName,
        revision_id: RevisionId,
        parent_id: Option<ContentId>,
        timestamp: DiskTimestamp,
    ) -> AnyhowResult<()>;

    fn change(&mut self, change: StateModification) -> Result<(), StateError> {
        match change {
            StateModification::Forgot(content_id) => self
                .forgot_with_children(content_id)
                .context(format!("Forgot (with children) content {}", content_id))?,
            StateModification::Add(content, relative_path, timestamp) => {
                self.add(content, relative_path, timestamp)?
            }
            StateModification::Update(
                content_id,
                file_name,
                new_revision_id,
                new_parent_id,
                new_timestamp,
            ) => self
                .update(
                    content_id,
                    file_name,
                    new_revision_id,
                    new_parent_id,
                    new_timestamp,
                )
                .context(format!("Update content {}", content_id))?,
        };

        Ok(())
    }

    fn forgot_with_children(&mut self, content_id: ContentId) -> AnyhowResult<()> {
        for child_id in self
            .direct_children_ids(content_id)
            .context(format!("Get children of {}", content_id))?
        {
            self.forgot_with_children(child_id)
                .context(format!("Forgot child {}", child_id))?;
        }

        self.forgot(content_id)
            .context(format!("Forgot {}", content_id))?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use trsync_core::instance::ContentId;

    use crate::tests::build_memory_state;
    use rstest::*;

    #[rstest]
    #[case(vec![(1, 1, "a.txt", None)], 1, "a.txt")]
    #[case(vec![(1, 1, "Folder", None), (2, 2, "a.txt", Some(1))], 2, "Folder/a.txt")]
    #[case(vec![(1, 1, "Folder1", None), (2, 2, "Folder2", Some(1)), (3, 3, "a.txt", Some(2))], 3, "Folder1/Folder2/a.txt")]
    fn test_content_path(
        #[case] raw_contents: Vec<(i32, i32, &str, Option<i32>)>,
        #[case] from_: i32,
        #[case] expected: &str,
    ) {
        // Given
        let state = build_memory_state(&raw_contents, None);

        // When
        let path = state.path(ContentId(from_)).unwrap();

        // Then
        let path_str = &Into::<PathBuf>::into(path).display().to_string();
        assert_eq!(path_str, expected);
    }
}
