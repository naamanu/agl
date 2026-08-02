def register(registry):
    def uppercase(args):
        return {"text": args["text"].upper()}

    registry.register_tool("uppercase", uppercase)
