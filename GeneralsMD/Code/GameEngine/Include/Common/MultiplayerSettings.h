// FILE: MultiplayerSettings.h /////////////////////////////////////////////////////////////////////////////
// Settings common to multiplayer games
// Author: Matthew D. Campbell, January 2002
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _MULTIPLAYERSETTINGS_H_
#define _MULTIPLAYERSETTINGS_H_

#include "GameClient/Color.h"
#include "Common/Money.h"

// FORWARD DECLARATIONS ///////////////////////////////////////////////////////////////////////////
struct FieldParse;
class MultiplayerSettings;

// PUBLIC /////////////////////////////////////////////////////////////////////////////////////////

class MultiplayerColorDefinition
{
public:
	MultiplayerColorDefinition();
	//-----------------------------------------------------------------------------------------------
	static const FieldParse m_colorFieldParseTable[];		///< the parse table for INI definition
	const FieldParse *getFieldParse( void ) const { return m_colorFieldParseTable; }

	inline AsciiString getTooltipName(void) const { return m_tooltipName; };
	inline RGBColor getRGBValue(void) const { return m_rgbValue; };
	inline RGBColor getRGBNightValue(void) const { return m_rgbValueNight; };
	inline Color getColor(void) const { return m_color; }
	inline Color getNightColor(void) const { return m_colorNight; }
	void setColor( RGBColor rgb );
	void setNightColor( RGBColor rgb );

	MultiplayerColorDefinition * operator =(const MultiplayerColorDefinition& other);

private:
	AsciiString m_tooltipName;	///< tooltip name for color combo box (AsciiString to pass to TheGameText->fetch())
	RGBColor m_rgbValue;						///< RGB color value
	Color m_color;
	RGBColor m_rgbValueNight;						///< RGB color value
	Color m_colorNight;
};

typedef std::map<Int, MultiplayerColorDefinition> MultiplayerColorList;
typedef std::map<Int, MultiplayerColorDefinition>::iterator MultiplayerColorIter;

// A list of values to display in the starting money dropdown
typedef std::vector< Money > MultiplayerStartingMoneyList;

//-------------------------------------------------------------------------------------------------
/** Multiplayer Settings container class
  *	Defines multiplayer settings */
//-------------------------------------------------------------------------------------------------
class MultiplayerSettings : public SubsystemInterface
{
public:

	MultiplayerSettings( void );

	virtual void init() { }
	virtual void update() { }
	virtual void reset() { }

	//-----------------------------------------------------------------------------------------------
	static const FieldParse m_multiplayerSettingsFieldParseTable[];		///< the parse table for INI definition
	const FieldParse *getFieldParse( void ) const { return m_multiplayerSettingsFieldParseTable; }

	// Color management --------------------
	MultiplayerColorDefinition * findMultiplayerColorDefinitionByName(AsciiString name);
	MultiplayerColorDefinition * newMultiplayerColorDefinition(AsciiString name);

	inline Int getStartCountdownTimerSeconds( void ) { return m_startCountdownTimerSeconds; }
	inline Int getMaxBeaconsPerPlayer( void ) { return m_maxBeaconsPerPlayer; }
	inline Bool isShroudInMultiplayer( void ) { return m_isShroudInMultiplayer; }
	inline Bool showRandomPlayerTemplate( void ) { return m_showRandomPlayerTemplate; }
	inline Bool showRandomStartPos( void ) { return m_showRandomStartPos; }
	inline Bool showRandomColor( void ) { return m_showRandomColor; }

	inline Int getNumColors( void ) 
	{
		if (m_numColors == 0) {
			m_numColors = m_colorList.size();
		}
		return m_numColors;
	}
	MultiplayerColorDefinition * getColor(Int which);


  const Money & getDefaultStartingMoney() const 
  { 
    DEBUG_ASSERTCRASH( m_gotDefaultStartingMoney, ("You must specify a default starting money amount in multiplayer.ini") );
    return m_defaultStartingMoney; 
  }

  const MultiplayerStartingMoneyList & getStartingMoneyList() const { return m_startingMoneyList; }

  void addStartingMoneyChoice( const Money & money, Bool isDefault );
    
private:
	Int m_initialCreditsMin;
	Int m_initialCreditsMax;
	Int m_startCountdownTimerSeconds;
	Int m_maxBeaconsPerPlayer;
	Bool m_isShroudInMultiplayer;
	Bool m_showRandomPlayerTemplate;
	Bool m_showRandomStartPos;
	Bool m_showRandomColor;

	MultiplayerColorList m_colorList;
	Int m_numColors;
	MultiplayerColorDefinition m_observerColor;
	MultiplayerColorDefinition m_randomColor;
  MultiplayerStartingMoneyList      m_startingMoneyList;
  Money                             m_defaultStartingMoney;
  Bool                              m_gotDefaultStartingMoney;
};

// singleton
extern MultiplayerSettings *TheMultiplayerSettings;

#endif
