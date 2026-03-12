// ChinookAIUpdate.h //////////
// Author: Steven Johnson, June 2002
 
#pragma once

#ifndef _ChinookAIUpdate_H_
#define _ChinookAIUpdate_H_

#include "GameLogic/AIStateMachine.h"
#include "GameLogic/Module/SupplyTruckAIUpdate.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------

//-------------------------------------------------------------------------------------------------
class ChinookAIUpdateModuleData : public SupplyTruckAIUpdateModuleData
{
public:
	AsciiString		m_ropeName;
  AsciiString   m_rotorWashParticleSystem;
  Real					m_rappelSpeed;
	Real					m_ropeDropSpeed;
	Real					m_ropeWidth;
	Real					m_ropeFinalHeight;
	Real					m_ropeWobbleLen;
	Real					m_ropeWobbleAmp;
	Real					m_ropeWobbleRate;
	RGBColor			m_ropeColor;
	UnsignedInt		m_numRopes;
	UnsignedInt		m_perRopeDelayMin;
	UnsignedInt		m_perRopeDelayMax;
	Real					m_minDropHeight;
	Bool					m_waitForRopesToDrop;
	Int						m_upgradedSupplyBoost;

	ChinookAIUpdateModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);
};

//-------------------------------------------------------------------------------------------------
enum ChinookFlightStatus // Stored in save file, don't renumber.  jba. 
{
	CHINOOK_TAKING_OFF				= 0,
	CHINOOK_FLYING						= 1,
	CHINOOK_DOING_COMBAT_DROP	= 2,
	CHINOOK_LANDING						= 3,
	CHINOOK_LANDED						= 4
};

//-------------------------------------------------------------------------------------------------
class ChinookAIUpdate : public SupplyTruckAIUpdate
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( ChinookAIUpdate, "ChinookAIUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( ChinookAIUpdate, ChinookAIUpdateModuleData )

public:

	ChinookAIUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual UpdateSleepTime update();
 	virtual void aiDoCommand(const AICommandParms* parms);
	virtual Bool chooseLocomotorSet(LocomotorSetType wst);
	// this is present solely for some transports to override, so that they can land before 
	// allowing people to exit...
	virtual AIFreeToExitType getAiFreeToExit(const Object* exiter) const;
	virtual Bool isAllowedToAdjustDestination() const;
	virtual ObjectID getBuildingToNotPathAround() const;

	// this is present for subclasses (eg, Chinook) to override, to
	// prevent supply-ferry behavior in some cases (eg, when toting passengers)
	virtual Bool isAvailableForSupplying() const;
	virtual Bool isCurrentlyFerryingSupplies() const;
	
	virtual Bool isIdle() const;

	const ChinookAIUpdateModuleData* friend_getData() const { return getChinookAIUpdateModuleData(); }
	void friend_setFlightStatus(ChinookFlightStatus a) { m_flightStatus = a; }

	void recordOriginalPosition( const Coord3D &pos ) { m_originalPos.set( &pos ); }
	const Coord3D* getOriginalPosition() const { return &m_originalPos; }
	
	Int ChinookAIUpdate::getUpgradedSupplyBoost() const;

protected:

	virtual AIStateMachine* makeStateMachine();

	virtual void privateCombatDrop( Object *target, const Coord3D& pos, CommandSourceType cmdSource );
	virtual void privateGetRepaired( Object *repairDepot, CommandSourceType cmdSource );///< get repaired at repair depot


	virtual void privateAttackObject( Object *victim, Int maxShotsToFire, CommandSourceType cmdSource );///< Extension.  Also tell occupants to attackObject
	virtual void privateAttackPosition( const Coord3D *pos, Int maxShotsToFire, CommandSourceType cmdSource );///< Extension.  Also tell occupants to attackPosition
	virtual void privateForceAttackObject( Object *victim, Int maxShotsToFire, CommandSourceType cmdSource );///< Extension.  Also tell occupants to forceAttackObject

  virtual void privateIdle(CommandSourceType cmdSource);

  void private___TellPortableStructureToAttackWithMe( Object *victim, Int maxShotsToFire, CommandSourceType cmdSource );


private:

	void setMyState( StateID cmd, Object* target, const Coord3D* pos, CommandSourceType cmdSource );
	void setAirfieldForHealing(ObjectID id);

	AICommandParmsStorage		m_pendingCommand;
	ChinookFlightStatus			m_flightStatus;
	ObjectID								m_airfieldForHealing;
	Coord3D									m_originalPos;
	Bool										m_hasPendingCommand;
};

#endif

