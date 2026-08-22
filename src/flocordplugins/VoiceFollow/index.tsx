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
        if (state.guildId !== followedGuildId) continue;

        // L'utilisateur a quitté la voc sans se déplacer → on ne suit pas
        if (!state.channelId) continue;

        const me = getCurrentUser();
        const myState = VoiceStateStore.getVoiceStateForUser(me.id);

        // Déjà dans le bon salon
        if (myState?.channelId === state.channelId) continue;

        if (!canJoinChannel(state.channelId)) {
            // Salon inaccessible : on se déco si l'option est activée et qu'on est en voc
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
    if (!user || !guildId) return;

    const targetVoiceState = VoiceStateStore.getVoiceStateForUser(user.id);
    if (!targetVoiceState?.channelId) return;

    const channel = ChannelStore.getChannel(targetVoiceState.channelId);
    if (!channel || channel.guild_id !== guildId) return;

    const isFollowed = followedUserId === user.id && followedGuildId === guildId;

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
                    followedGuildId = guildId;
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
