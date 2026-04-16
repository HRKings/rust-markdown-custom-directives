-- Each invocation of "spawn" returns markdown containing 10 more {{spawn}} directives.
-- With budget limit of 1024, this should exhaust the budget quickly.
mdx.register_directive("spawn", function(inv)
    local parts = {}
    for i = 1, 10 do
        parts[i] = "{{spawn}}"
    end
    return { type = "markdown", value = table.concat(parts, " ") }
end)
