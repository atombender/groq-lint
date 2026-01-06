// Example TypeScript file with GROQ queries
// Run: npx eslint queries.ts

// Type definition for the groq tag
const groq = (strings: TemplateStringsArray): string => strings.join("");

// Good: Simple filter query
const postsQuery = groq`*[_type == "post"]`;

// Good: Query with projection
const postTitlesQuery = groq`*[_type == "post"]{
  title,
  slug
}`;

// Bad: Join inside filter (will trigger join_in_filter)
const postsByAuthorQuery = groq`*[_type == "post" && author->name == "Alice"]`;

// Bad: Using -> to get _id (will trigger join_to_get_id)
const authorIdsQuery = groq`*[_type == "post"]{
  "authorId": author->_id
}`;

// Good: Correlated subquery with parent reference
const draftsQuery = groq`*[_type == "post"]{
  "draft": *[_id == "drafts." + ^._id][0]
}`;

export {
  postsQuery,
  postTitlesQuery,
  postsByAuthorQuery,
  authorIdsQuery,
  draftsQuery,
};
