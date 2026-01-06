// Assume groq is imported from a library like @sanity/client or groq
const groq = String.raw;

// Good: Simple filter query
const postsQuery = groq`*[_type == "post"]`;

// Good: Query with projection
const postTitlesQuery = groq`*[_type == "post"]{
  title,
  "authorName": author->name
}`;

// Good: Query with ordering
const recentPostsQuery = groq`*[_type == "post"] | order(publishedAt desc)`;

// Bad: Join inside filter (will trigger join_in_filter)
const postsByAuthorQuery = groq`*[_type == "post" && author->name == "Alice"]`;

// Bad: Computed value in filter (will trigger computed_value_in_filter)
const searchQuery = groq`*[title + " " + subtitle == "Hello World"]`;

// Bad: Deep pagination (will trigger deep_pagination)
const pagedQuery = groq`*[_type == "post"][2000...2010]`;

// Bad: Using -> to get _id (will trigger join_to_get_id)
const authorIdsQuery = groq`*[_type == "post"]{
  "authorId": author->_id
}`;

// Good: Correlated subquery with parent reference is fine
const draftsQuery = groq`*[_type == "post"]{
  "draft": *[_id == "drafts." + ^._id][0]
}`;

export {
  postsQuery,
  postTitlesQuery,
  recentPostsQuery,
  postsByAuthorQuery,
  searchQuery,
  pagedQuery,
  authorIdsQuery,
  draftsQuery,
};
