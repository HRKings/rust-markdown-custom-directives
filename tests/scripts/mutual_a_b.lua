mdx.register_directive("ping", function(inv)
    return { type = "markdown", value = "ping->{{pong}}" }
end)

mdx.register_directive("pong", function(inv)
    return { type = "markdown", value = "pong->{{ping}}" }
end)
