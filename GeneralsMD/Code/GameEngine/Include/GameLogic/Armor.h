// FILE: Armor.h /////////////////////////////////////////////////////////////////////////////////
// Author: Steven Johnson, March 2002
// Desc:   Damage Multiplier Descriptions
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _Armor_H_
#define _Armor_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/NameKeyGenerator.h"
#include "Common/STLTypedefs.h"
#include "GameLogic/Damage.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class ArmorStore;

//-------------------------------------------------------------------------------------------------
/** 
	An Armor encapsulates the a particular type of actual modifier to damage taken, in order
	to simulate different materials, and to help make game balance easier to adjust.
*/
//-------------------------------------------------------------------------------------------------
class ArmorTemplate
{
public:

	ArmorTemplate();

	void clear();

	/**
		This is the real "meat" of the class: given a damage type and amount, adjust the damage
		and return the amount that should be dealt. 
	*/	
	Real adjustDamage(DamageType t, Real damage) const;

	static void parseArmorCoefficients( INI* ini, void *instance, void* /* store */, const void* userData );

protected:

private:
	Real						m_damageCoefficient[DAMAGE_NUM_TYPES];	///< modifiers to damage
};  

//-------------------------------------------------------------------------------------------------
class Armor
{
public:

	inline Armor(const ArmorTemplate* tmpl = NULL) : m_template(tmpl) 
	{ 
	}

	inline Real adjustDamage(DamageType t, Real damage) const
	{
		return m_template ? m_template->adjustDamage(t, damage) : damage;
	}

	inline void clear()
	{
		m_template = NULL;
	}

private:
	const ArmorTemplate* m_template;		///< the kind of armor this is
};

//------------------------------------------------------------------------------------------------
/** Interface class for TheArmorStore, which is just a convenient place for us to
	* store each Armor we read from INI together in a list, with some access
	* methods for finding Armors */
//-------------------------------------------------------------------------------------------------
class ArmorStore : public SubsystemInterface
{

public:

	ArmorStore();
	~ArmorStore();

	void init() { }
	void reset() { }
	void update() { }

	/**
		Find the Armor with the given name. If no such Armor exists, return null.
	*/
	const ArmorTemplate* findArmorTemplate(AsciiString name) const;

	inline Armor makeArmor(const ArmorTemplate *tmpl) const
	{
		return Armor(tmpl);	// my, that was easy
	}

	static void parseArmorDefinition(INI* ini);

private:

	typedef std::hash_map< NameKeyType, ArmorTemplate, rts::hash<NameKeyType>, rts::equal_to<NameKeyType> > ArmorTemplateMap;
	ArmorTemplateMap m_armorTemplates;

};

// EXTERNALS //////////////////////////////////////////////////////////////////////////////////////
extern ArmorStore *TheArmorStore;

#endif // _Armor_H_

