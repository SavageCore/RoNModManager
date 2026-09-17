// git-cliff drops any subject that is not "type(scope): description" (see
// packaging/flatpak/cliff-appstream.toml), so commitlint guards the changelog.
export default {
  extends: ["@commitlint/config-conventional"],
  rules: {
    // Bodies here are prose paragraphs, not wrapped to 100 columns.
    "body-max-line-length": [0],
    "footer-max-line-length": [0],
  },
};
