import { LitElement, html, css, customElement, property } from "lit-element";
import "@elements/core/dividers/spacer-fourty";
import "@elements/entry/user/_common/auth-page";

const STR_TITLE = "Sign Up - Step 2";

@customElement("page-register-step2")
export class _ extends LitElement {
    static get styles() {
        return [
            css`
                h1 {
                    font-size: 32px;
                    font-weight: 900;
                    color: #5662a3;
                }
                .inside-wrapper {
                    max-width: 650px;
                    display: grid;
                    grid-template-columns: 1fr 1fr;
                    align-items: start;
                    gap: 32px;
                }
                ::slotted([slot=checkbox]),
                ::slotted([slot=committed-to-privacy]),
                ::slotted([slot=submit]) {
                    grid-column: 1 / -1;
                }
                ::slotted([slot="committed-to-privacy"]) {
                    width: 60%;
                }
            `,
        ];
    }

    render() {
        return html`
            <auth-page img="entry/user/side/step-2.webp">
                <h1>${STR_TITLE}</h1>
                <div class="inside-wrapper">
                    <slot name="location"></slot>
                    <slot name="language"></slot>
                    <slot name="persona"></slot>
                    <slot name="organization"></slot>
                    <slot name="checkbox"> </slot>
                    <slot name="committed-to-privacy"></slot>
                    <slot name="submit"></slot>
                </div>
            </auth-page>
        `;
    }
}
