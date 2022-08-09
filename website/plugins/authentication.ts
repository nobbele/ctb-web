import { defineNuxtPlugin, useState } from '#app'
import { useToken } from '~~/composables/useToken';
import { ApiType, currentApiType, withApi, UserData } from '~~/composables/withApi';

export default defineNuxtPlugin(async _app => {
    const token = useToken();

    const apiType = useCookie<ApiType>('apiType', { sameSite: 'strict', maxAge: 60 * 60 * 24 * 90 });
    if (apiType.value == undefined) apiType.value = currentApiType;

    if (apiType.value != currentApiType) {
        token.value = null;
        apiType.value = currentApiType;
    }

    const userdata = useState<UserData | null>('userdata', () => null);
    const refreshUserdata = async () => {
        if (token.value != null) {
            userdata.value = await withApi().getMe();
            if (userdata.value == null) {
                throw 'Invalid Token';
            }
        } else {
            userdata.value = null;
        }
    };
    await refreshUserdata();

    return {
        provide: {
            userdata,
            async syncToken(new_token: string) {
                token.value = new_token;
                setTimeout(() => {
                    refreshUserdata()
                }, 100);
            },
            async unsyncToken() {
                token.value = null;
                setTimeout(() => {
                    refreshUserdata()
                }, 100);
            },
            refreshUserdata,
        }
    }
})