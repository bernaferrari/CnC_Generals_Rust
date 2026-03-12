
// jkmcd
#pragma once

struct BorderColor
{
	char *m_colorName;
	long m_borderColor;
};

const BorderColor BORDER_COLORS[] = 
{
	{ "Orange",					0xFFFF8700, },
	{ "Green",					0xFF00FF00, },
	{ "Blue",						0xFF0000FF, },
	{ "Cyan",						0xFF00FFFF, },
	{ "Magenta",				0xFFFF00FF, },
	{ "Yellow",					0xFFFFFF00, },
	{ "Purple",					0xFF9E00FF, },
	{ "Pink",						0xFFFF8670, },
};

const long BORDER_COLORS_SIZE = sizeof(BORDER_COLORS) / sizeof (BORDER_COLORS[0]);
