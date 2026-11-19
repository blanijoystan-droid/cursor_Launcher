/**
 * Validates and resolves user input into either a direct URL or a Google Search URL.
 * 
 * Rules:
 * - If input starts with http://, https://, or localhost -> routes directly to that URL.
 * - If input has no spaces and matches a domain pattern (e.g. github.com, news.ycombinator.com) -> routes directly to https://[domain].
 * - If input contains spaces or is a keyword without a URL/domain (e.g. "how to center a div", "deepseek", "weather") -> routes to https://www.google.com/search?q=[URL_ENCODED_QUERY].
 */
export function resolveQuery(rawInput: string): string {
  const query = rawInput.trim();
  if (!query) {
    return "https://www.google.com";
  }

  // If query contains whitespace, it's definitely a search query
  if (/\s/.test(query)) {
    return `https://www.google.com/search?q=${encodeURIComponent(query)}`;
  }

  // Direct scheme
  if (/^https?:\/\//i.test(query)) {
    return query;
  }

  // Localhost or IP-based local server
  if (/^(localhost|127\.0\.0\.1)(:\d+)?(\/.*)?$/i.test(query)) {
    return `http://${query}`;
  }

  // Domain structure: domain.tld or sub.domain.tld optionally with port and path
  // Must have at least one dot, and TLD must be at least 2 alpha characters
  const domainPattern = /^[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*\.[a-zA-Z]{2,}(?::\d+)?(?:\/.*)?$/i;

  if (domainPattern.test(query)) {
    return `https://${query}`;
  }

  // Otherwise, fallback to Google search (e.g. "deepseek", "claude", "so")
  return `https://www.google.com/search?q=${encodeURIComponent(query)}`;
}
