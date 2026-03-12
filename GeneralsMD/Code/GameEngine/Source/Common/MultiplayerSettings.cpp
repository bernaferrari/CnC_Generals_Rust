// FILE: MultiplayerSettings.cpp ///////////////////////////////////////////////////////////////////////////
// The MultiplayerSettings object
// Author: Matthew D. Campbell, January 2002
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#define DEFINE_TERRAIN_LOD_NAMES
#define DEFINE_TIME_OF_DAY_NAMES

#include "Common/MultiplayerSettings.h"
#include "Common/INI.h"
#include "GameNetwork/GameInfo.h" // for PLAYERTEMPLATE_*

// PUBLIC DATA ////////////////////////////////////////////////////////////////////////////////////
MultiplayerSettings *TheMultiplayerSettings = NULL;				///< The MultiplayerSettings singleton

///////////////////////////////////////////////////////////////////////////////////////////////////
// PRIVATE DATA ///////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
const FieldParse MultiplayerColorDefinition::m_colorFieldParseTable[] = 
{

	{ "TooltipName",	INI::parseAsciiString,	NULL,	offsetof( MultiplayerColorDefinition, m_tooltipName ) },
	{ "RGBColor",			INI::parseRGBColor,			NULL,	offsetof( MultiplayerColorDefinition, m_rgbValue ) },
	{ "RGBNightColor",			INI::parseRGBColor,		NULL,	offsetof( MultiplayerColorDefinition, m_rgbValueNight ) },
	{ NULL,					NULL,						NULL,						0 }  // keep this last

};

const FieldParse MultiplayerSettings::m_multiplayerSettingsFieldParseTable[] = 
{

	{ "StartCountdownTimer",			INI::parseInt,	NULL,	offsetof( MultiplayerSettings, m_startCountdownTimerSeconds ) },
	{ "MaxBeaconsPerPlayer",			INI::parseInt,	NULL,	offsetof( MultiplayerSettings, m_maxBeaconsPerPlayer ) },
	{ "UseShroud",								INI::parseBool,	NULL,	offsetof( MultiplayerSettings, m_isShroudInMultiplayer ) },
	{ "ShowRandomPlayerTemplate",	INI::parseBool,	NULL,	offsetof( MultiplayerSettings, m_showRandomPlayerTemplate ) },
	{ "ShowRandomStartPos",				INI::parseBool,	NULL,	offsetof( MultiplayerSettings, m_showRandomStartPos ) },
	{ "ShowRandomColor",					INI::parseBool,	NULL,	offsetof( MultiplayerSettings, m_showRandomColor ) },

	{ NULL,					NULL,						NULL,						0 }  // keep this last

};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
MultiplayerSettings::MultiplayerSettings()
{
	m_maxBeaconsPerPlayer = 3;
	//

	m_startCountdownTimerSeconds = 0;
	m_numColors = 0;
	m_isShroudInMultiplayer = TRUE;
	m_showRandomPlayerTemplate = TRUE;
	m_showRandomStartPos = TRUE;
	m_showRandomColor = TRUE;
	
	m_observerColor;
	m_randomColor;

  m_gotDefaultStartingMoney = false;
}  // end MultiplayerSettings

MultiplayerColorDefinition::MultiplayerColorDefinition()
{
	m_tooltipName.clear();
	m_rgbValue.setFromInt(0xFFFFFFFF);
	m_rgbValueNight=m_rgbValue;
	m_color = 0xFFFFFFFF;
	m_colorNight = m_color;
}

MultiplayerColorDefinition * MultiplayerSettings::getColor(Int which)
{
	if (which == PLAYERTEMPLATE_RANDOM)
	{
		return &m_randomColor;
	}
	else if (which == PLAYERTEMPLATE_OBSERVER)
	{
		return &m_observerColor;
	}
	else if (which < 0 || which >= getNumColors())
	{
		return NULL;
	}

	return &m_colorList[which];
}

MultiplayerColorDefinition * MultiplayerSettings::findMultiplayerColorDefinitionByName(AsciiString name)
{
	MultiplayerColorIter iter = m_colorList.begin();

	while (iter != m_colorList.end())
	{
		if (iter->second.getTooltipName() == name)
			return &(iter->second);

		++iter;
	}

	return NULL;
}

MultiplayerColorDefinition * MultiplayerSettings::newMultiplayerColorDefinition(AsciiString name)
{
 	MultiplayerColorDefinition tmp;
	Int numColors = getNumColors();

	m_colorList[numColors] = tmp;
	m_numColors = m_colorList.size();

	return &m_colorList[numColors];
}

void MultiplayerSettings::addStartingMoneyChoice( const Money & money, Bool isDefault )
{
  m_startingMoneyList.push_back( money );
  if ( isDefault )
  {
    DEBUG_ASSERTCRASH( !m_gotDefaultStartingMoney, ("Cannot have more than one default MultiplayerStartingMoneyChoice") );
    m_defaultStartingMoney = money;
    m_gotDefaultStartingMoney = true;
  }
}

MultiplayerColorDefinition * MultiplayerColorDefinition::operator =(const MultiplayerColorDefinition& other)
{
	m_tooltipName = other.getTooltipName();
	m_rgbValue = other.getRGBValue();
	m_color = other.getColor();
	m_rgbValueNight = other.getRGBNightValue();
	m_colorNight = other.getNightColor();

	return this;
}

void MultiplayerColorDefinition::setColor( RGBColor rgb )
{
	m_color = rgb.getAsInt() | 0xFF << 24;
}

void MultiplayerColorDefinition::setNightColor( RGBColor rgb )
{
	m_colorNight = rgb.getAsInt() | 0xFF << 24;
}

