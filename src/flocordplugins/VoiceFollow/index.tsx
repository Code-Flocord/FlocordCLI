import { definePlugin, OptionType } from "@utils/types";
import { FluxDispatcher } from "@webpack/common";
import { addContextMenuPatch, removeContextMenuPatch, NavContextMenuPatchCallback } from "@api/ContextMenu";
import { getCurrentUser } from "@utils/discord";
import { PermissionsBits } from "@utils/discord";
import { VoiceStateStore, ChannelStore, PermissionStore } from "@webpack/common";
import { Menu } from "@webpack/common";
import { selectVoiceChannel } from "@utils/discord";

interface VoiceStateUpdate {
    userId: string;
    guildId: string | null;
    channelId: string | null;
}

interface VoiceStateUpdateEvent {
    voiceStates: VoiceStateUpdate[];
}

let followedUserId: string | null = null;
let followedGuildId: string | null = null;

function canJoinChannel(channelId: string): boolean {
    const channel = ChannelStore.getChannel(channelId);
    if (!channel) return false;
    return PermissionStore.can(PermissionsBits.VIEW_CHANNEL | PermissionsBits.CONNECT, channel);
}

function handleVoiceStateUpdates({ voiceStates }: VoiceStateUpdateEvent) {
    if (!followedUserId || !followedGuildId) return;

    for (const state of voiceStates) {
        if (state.userId !== followedUserId) continue;

        // Si guildId null → suivi universel (MP inclus), sinon filtre par serveur
        if (followedGuildId !== null && state.guildId !== followedGuildId) continue;

        if (!state.channelId) continue;

        const me = getCurrentUser();
        const myState = VoiceStateStore.getVoiceStateForUser(me.id);

        if (myState?.channelId === state.channelId) continue;

        const channel = ChannelStore.getChannel(state.channelId);

        // Vérification des permissions uniquement pour les salons de serveur
        if (channel?.guild_id && !canJoinChannel(state.channelId)) {
            if (settings.store.disconnectOnInaccessible && myState?.channelId) {
                selectVoiceChannel(null);
            }
            continue;
        }

        selectVoiceChannel(state.channelId);
        break;
    }
}

const userContextPatch: NavContextMenuPatchCallback = (children, { user, guildId }) => {
    if (!user) return;

    const resolvedGuildId = guildId ?? null;
    const isFollowed = followedUserId === user.id && followedGuildId === resolvedGuildId;

    children.push(
        <Menu.MenuItem
            id="flocord-voice-follow"
            label={isFollowed ? "Ne plus suivre en VOC" : "Suivre en VOC"}
            action={() => {
                if (isFollowed) {
                    followedUserId = null;
                    followedGuildId = null;
                } else {
                    followedUserId = user.id;
                    followedGuildId = resolvedGuildId;
                }
            }}
        />
    );
};

const settings = definePluginSettings({
    disconnectOnInaccessible: {
        type: OptionType.BOOLEAN,
        description: "Se déconnecter de la vocal si le salon rejoint par le suivi est inaccessible",
        default: false,
    },
});

export default definePlugin({
    name: "VoiceFollow",
    description: "Rejoint automatiquement le salon vocal d'un utilisateur quand il change de salon (même serveur requis, amis inutiles)",
    authors: [{ name: "Flocord", id: 0n }],
    settings,

    start() {
        addContextMenuPatch("user-context", userContextPatch);
        FluxDispatcher.subscribe("VOICE_STATE_UPDATES", handleVoiceStateUpdates);
    },

    stop() {
        removeContextMenuPatch("user-context", userContextPatch);
        FluxDispatcher.unsubscribe("VOICE_STATE_UPDATES", handleVoiceStateUpdates);
        followedUserId = null;
        followedGuildId = null;
    },
});
