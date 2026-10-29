using System.Text.Json;
using System.Text.Json.Nodes;
using GitaView.Native.Contracts;

namespace GitaView.Native.Tests;

public class WidgetCacheValidationFacts
{
    internal const string ValidJson = """
        {"schemaVersion":1,"sessionId":"cache","generation":1,"stateRevision":1,"settingsRevision":1,
         "refreshState":"ready","refreshPolicy":{"enabled":false,"intervalMinutes":5},
         "counts":{"total":0,"synced":0,"localAhead":0,"remoteAhead":0,"diverged":0,"noRemote":0,"error":0,"pending":0},
         "compact":{"synced":0,"syncable":0,"needsAttention":0,"noRemote":0},"attention":[]}
        """;

    [Theory]
    [InlineData("counts")]
    [InlineData("compact")]
    [InlineData("refreshPolicy")]
    [InlineData("attention")]
    [InlineData("sessionId")]
    public void null_required_cache_members_are_rejected(string property)
    {
        var damaged = JsonNode.Parse(ValidJson)!;
        damaged[property] = null;
        Assert.Throws<JsonException>(() => ContractsJson.Decode<WidgetSnapshot>(damaged.ToJsonString()));
    }

    [Fact]
    public void null_attention_entries_are_rejected()
    {
        var damaged = JsonNode.Parse(ValidJson)!;
        damaged["attention"] = new JsonArray((JsonNode?)null);
        Assert.Throws<JsonException>(() => ContractsJson.Decode<WidgetSnapshot>(damaged.ToJsonString()));
    }
}
