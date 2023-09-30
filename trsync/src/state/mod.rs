use std::path::PathBuf;

use anyhow::{Context, Result};

use trsync_core::{
    content::Content,
    instance::{ContentFileName, ContentId, DiskTimestamp, RevisionId},
};

use crate::path::ContentPath;

use self::modification::StateModification;

pub mod memory;
pub mod modification;

pub trait State {
    fn known(&self, id: ContentId) -> Result<bool>;
    fn get(&self, id: ContentId) -> Result<Option<&Content>>;
    fn content_id_for_path(&self, path: PathBuf) -> Result<Option<ContentId>>;
    // Path must be build on demand because parent hierarchy can change
    fn path(&self, id: ContentId) -> Result<ContentPath>;
    // FIXME BS NOW : Iter
    // pub trait Trait {
    //     type Iter<'a>: Iterator<Item = &'a Content> + 'a
    //     where
    //         Self: 'a;
    //     fn contents(&self) -> Result<Self::Iter<'_>>;
    // }

    // impl Trait for Map {
    //     type Iter<'a> = Values<'a, ContentId, Content>;
    //     fn contents(&self) -> Result<Self::Iter<'_>> {
    //         Ok(self.contents.values())
    //     }
    // }
    /// Return iterable of `&Contents` ordered by `ContentType::Folder` first
    fn contents(&self) -> Result<Vec<&Content>>;
    fn children_ids(&self, content_id: ContentId) -> Result<Vec<ContentId>>;
    fn forgot(&mut self, content_id: ContentId) -> Result<()>;
    fn add(&mut self, content: Content) -> Result<()>;
    fn rename(
        &mut self,
        content_id: ContentId,
        file_name: ContentFileName,
        parent_id: Option<ContentId>,
    ) -> Result<()>;
    fn update(
        &mut self,
        content_id: ContentId,
        file_name: ContentFileName,
        revision_id: RevisionId,
        parent_id: Option<ContentId>,
        timestamp: DiskTimestamp,
    ) -> Result<()>;

    fn change(&mut self, change: StateModification) -> Result<()> {
        match change {
            StateModification::Forgot(content_id) => self
                .forgot_with_children(content_id)
                .context(format!("Forgot (with children) content {}", content_id))?,
            StateModification::Add(content) => {
                let content_id = content.id();
                self.add(content)
                    .context(format!("Add content {}", content_id))?
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
            StateModification::Rename(content_id, file_name, parent_id) => {
                self.rename(content_id, file_name, parent_id)
                    .context(format!("Rename content {}", content_id))?;
            }
        };

        Ok(())
    }

    fn forgot_with_children(&mut self, content_id: ContentId) -> Result<()> {
        for child_id in self
            .children_ids(content_id)
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
