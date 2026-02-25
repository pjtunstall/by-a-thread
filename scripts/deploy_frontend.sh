curl -X POST \
  -H "Authorization: token YOUR_GITHUB_TOKEN" \
  -H "Accept: application/vnd.github.v3+json" \
  https://api.github.com/repos/pjtunstall/by-a-thread/dispatches \
  -d '{"event_type":"deploy_itch"}'