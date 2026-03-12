#pragma once

#include "Common/MessageStream.h"
#include "Common/GameMemory.h"

//----------------------------------------------------------------------------
class GameMessageParserArgumentType : public MemoryPoolObject
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(GameMessageParserArgumentType, "GameMessageParserArgumentType")		
public:
	GameMessageParserArgumentType(GameMessageArgumentDataType type, Int argCount);
	//virtual ~GameMessageParserArgumentType();

	GameMessageParserArgumentType *getNext();
	void setNext(GameMessageParserArgumentType *next);
	Int getArgCount();
	GameMessageArgumentDataType getType();

protected:
	GameMessageParserArgumentType*	m_next;
	GameMessageArgumentDataType			m_type;
	Int															m_argCount;
};

//----------------------------------------------------------------------------
inline GameMessageParserArgumentType * GameMessageParserArgumentType::getNext() 
{
	return m_next;
}

//----------------------------------------------------------------------------
inline void GameMessageParserArgumentType::setNext(GameMessageParserArgumentType *next) 
{
	m_next = next;
}

//----------------------------------------------------------------------------
inline GameMessageArgumentDataType GameMessageParserArgumentType::getType() 
{
	return m_type;
}

//----------------------------------------------------------------------------
inline Int GameMessageParserArgumentType::getArgCount() 
{
	return m_argCount;
}

//----------------------------------------------------------------------------
//----------------------------------------------------------------------------
//----------------------------------------------------------------------------
class GameMessageParser : public MemoryPoolObject
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(GameMessageParser, "GameMessageParser")		
public:
	GameMessageParser();
	GameMessageParser(GameMessage *msg);
	//virtual ~GameMessageParser();

	GameMessageParserArgumentType *getFirstArgumentType();
	void addArgType(GameMessageArgumentDataType type, Int argCount);
	Int getNumTypes();

protected:
	GameMessageParserArgumentType *m_first, *m_last;
	Int m_argTypeCount;
};

//----------------------------------------------------------------------------
inline GameMessageParserArgumentType * GameMessageParser::getFirstArgumentType() 
{
	return m_first;
}

//----------------------------------------------------------------------------
inline Int GameMessageParser::getNumTypes() 
{
	return m_argTypeCount;
}

