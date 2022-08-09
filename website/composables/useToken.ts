import { useCookie } from "#app";

export function useToken() {
    const token = useCookie<string | null>('token', { sameSite: 'strict', maxAge: 60 * 60 * 24 * 90, default: () => null, });
    if (token.value === undefined) token.value = null;

    return token;
}