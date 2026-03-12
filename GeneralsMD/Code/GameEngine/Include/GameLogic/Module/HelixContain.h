///////////////////////////////////////////////////////////////////////////////////////////////////
//
//  FILE: HelixContain.h ////////////////////////////////////////////////////////////////////////
//  Author: Mark Lorenzen, April, 2003
//
//  Desc:   Contain module that acts as transport normally, but 
//
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __HELIX_CONTAIN_H_
#define __HELIX_CONTAIN_H_

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/TransportContain.h"

typedef std::vector<AsciiString> TemplateNameList;
typedef std::vector<AsciiString>::const_iterator TemplateNameIterator;

//-------------------------------------------------------------------------------------------------
class HelixContainModuleData : public TransportContainModuleData
{
public:

	HelixContainModuleData();

	TemplateNameList m_payloadTemplateNameData;
  Bool m_drawPips;


	static void buildFieldParse(MultiIniFieldParse& p);
	static void parseInitialPayload( INI* ini, void *instance, void *store, const void* /*userData*/ );
};

//-------------------------------------------------------------------------------------------------
class HelixContain : public TransportContain
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( HelixContain, "HelixContain" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( HelixContain, HelixContainModuleData )

	virtual void onBodyDamageStateChange( const DamageInfo* damageInfo, 
																				BodyDamageType oldState, 
																				BodyDamageType newState);  ///< state change callback
public:

	HelixContain( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration
	
	virtual OpenContain *asOpenContain() { return this; }  ///< treat as open container
	virtual Bool isHealContain() const { return false; } ///< true when container only contains units while healing (not a transport!)
	virtual Bool isTunnelContain() const { return FALSE; }
	virtual Bool isImmuneToClearBuildingAttacks() const { return true; }
  virtual Bool isSpecialOverlordStyleContainer() const {return TRUE;}

	virtual void onDie( const DamageInfo *damageInfo );  ///< the die callback
	virtual void onDelete( void );	///< Last possible moment cleanup
	virtual void onCapture( Player *oldOwner, Player *newOwner ); 
	virtual void onObjectCreated();
  virtual void onContaining( Object *obj, Bool wasSelected  );
  virtual void onRemoving( Object *obj );


  virtual UpdateSleepTime update();							///< called once per frame

  
 // virtual void onContaining( Object *obj, Bool wasSelected );		///< object now contains 'obj'
//	virtual void onRemoving( Object *obj );			///< object no longer contains 'obj'

	virtual Bool isValidContainerFor(const Object* obj, Bool checkCapacity) const;
	virtual void addToContain( Object *obj );				///< add 'obj' to contain list
	virtual void addToContainList( Object *obj );		///< The part of AddToContain that inheritors can override (Can't do whole thing because of all the private stuff involved)
	virtual void removeFromContain( Object *obj, Bool exposeStealthUnits = FALSE );	///< remove 'obj' from contain list
	//virtual void removeAllContained( Bool exposeStealthUnits = FALSE );				///< remove all objects on contain list
	virtual Bool isEnclosingContainerFor( const Object *obj ) const;	///< Does this type of Contain Visibly enclose its contents?
	virtual Bool isPassengerAllowedToFire(  ObjectID id = INVALID_ID  ) const;	///< Hey, can I shoot out of this container?

	// Friend for our Draw module only.
	virtual const Object *friend_getRider() const; ///< Damn.  The draw order dependency bug for riders means that our draw module needs to cheat to get around it.
	
	///< if my object gets selected, then my visible passengers should, too
	///< this gets called from
	virtual void clientVisibleContainedFlashAsSelected(); 

  virtual void redeployOccupants();

	virtual Bool getContainerPipsToShow(Int& numTotal, Int& numFull)
  {
    if ( getHelixContainModuleData()->m_drawPips == FALSE )
    {
      numTotal = 0;
      numFull = 0;
      return FALSE;
    }

    return ContainModuleInterface::getContainerPipsToShow( numTotal, numFull );
  }
  
	virtual void createPayload();

private:
  void parseInitialPayload( INI* ini, void *instance, void *store, const void* /*userData*/ );
  Object *getPortableStructure( void );
  ObjectID  m_portableStructureID;

};

#endif

