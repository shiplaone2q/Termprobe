package termprobe

import "time"

type locatorNode struct {
	kind       string
	text       *string
	regex      *bool
	whitespace *Whitespace
	style      *TextStyle
	link       *string
	full       *bool
	direction  *Direction
	within     *uint32
	left       *uint32
	right      *uint32
	input      *uint32
	has        *uint32
	hasNot     *uint32
	occurrence any
}

type locatorQuery struct {
	nodes []locatorNode
	root  uint32
}

type LocatorFilterOptions struct {
	Has    *Locator
	HasNot *Locator
}

type LinkSelectorOptions struct {
	Full      bool
	Direction Direction
}

// Locator is an immutable query, resolved against the current grid on each operation.
// Invalid selector options are reported when an operation resolves the locator.
type Locator struct {
	client *Client
	query  locatorQuery
}

func (client *Client) GetByText(text string, options TextSelectorOptions) *Locator {
	return newTextLocator(client, nil, text, options)
}

func (client *Client) GetByStyle(style TextStyle, options StyleSelectorOptions) *Locator {
	return newStyleLocator(client, nil, style, options)
}

func (client *Client) GetByLink(uri string, options LinkSelectorOptions) *Locator {
	return newLinkLocator(client, nil, uri, options)
}

func cloneLocatorQuery(source locatorQuery) locatorQuery {
