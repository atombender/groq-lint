// Example TypeScript file with GROQ queries
// Run: npx eslint queries.ts

// Type definitions (normally from 'groq' package)
const groq = (strings: TemplateStringsArray): string => strings.join("");
const defineQuery = <T extends string>(query: T): T => query;

// === Using groq`...` template tag ===

// Good: Simple filter query
const postsQuery = groq`*[_type == "post"]`;

// Bad: Join inside filter
const postsByAuthorQuery = groq`*[_type == "post" && author->name == "Alice"]`;

// === Using defineQuery() ===

// Good: Simple query with defineQuery
const articlesQuery = defineQuery(`*[_type == "article"]`);

// Bad: Join inside filter with defineQuery (template literal)
const articlesByAuthorQuery = defineQuery(`*[_type == "article" && author->name == "Bob"]`);

// Bad: Using -> to get _id with defineQuery (string literal)
const authorIdsQuery = defineQuery('*[_type == "post"]{ "authorId": author->_id }');

// Good: Correlated subquery with parent reference
const draftsQuery = defineQuery(`*[_type == "post"]{
  "draft": *[_id == "drafts." + ^._id][0]
}`);

export {
  postsQuery,
  postsByAuthorQuery,
  articlesQuery,
  articlesByAuthorQuery,
  authorIdsQuery,
  draftsQuery,
};
