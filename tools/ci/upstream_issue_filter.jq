add
| map(select((.body // "") | contains($marker)))
| .[0].number // empty
