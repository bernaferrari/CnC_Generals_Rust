// DrawGroupInfo.cpp //////////////////////////////////////////////////////////////////////////////
// Author: John McDonald, October 2002
///////////////////////////////////////////////////////////////////////////////////////////////////
 
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "GameClient/DrawGroupInfo.h"

// Useful defaults.

DrawGroupInfo::DrawGroupInfo()
{
	m_fontName = "Arial";
	m_fontSize = 10;
	m_fontIsBold = FALSE;

	m_usePlayerColor = TRUE;
	m_colorForText = GameMakeColor(255, 255, 255, 255);
	m_colorForTextDropShadow = GameMakeColor(0, 0, 0, 255);

	m_dropShadowOffsetX = -1;
	m_dropShadowOffsetY = -1;

	m_percentOffsetX = -0.05f;
	m_usingPixelOffsetX = FALSE;

	m_pixelOffsetY = -10;
	m_usingPixelOffsetY = TRUE;
}

DrawGroupInfo *TheDrawGroupInfo = NULL;