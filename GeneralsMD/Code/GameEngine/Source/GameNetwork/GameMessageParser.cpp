#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "GameNetwork/GameMessageParser.h"

//----------------------------------------------------------------------------
GameMessageParser::GameMessageParser() 
{
	m_first = NULL;
	m_argTypeCount = 0;
}

//----------------------------------------------------------------------------
GameMessageParser::GameMessageParser(GameMessage *msg) 
{
	m_first = NULL;
	m_argTypeCount = 0;

	UnsignedByte argCount = msg->getArgumentCount();
	GameMessageArgumentDataType lasttype = ARGUMENTDATATYPE_UNKNOWN;
	Int thisTypeCount = 0;

	for (UnsignedByte i = 0; i < argCount; ++i) {
		GameMessageArgumentDataType type = msg->getArgumentDataType(i);
		if (type != lasttype) {
			if (thisTypeCount > 0) {
				addArgType(lasttype, thisTypeCount);
				++m_argTypeCount;
			}
			lasttype = type;
			thisTypeCount = 0;
		}
		++thisTypeCount;
	}
	if (thisTypeCount > 0) {
		addArgType(lasttype, thisTypeCount);
		++m_argTypeCount;
	}
}

//----------------------------------------------------------------------------
GameMessageParser::~GameMessageParser() 
{
	GameMessageParserArgumentType *temp = NULL;
	while (m_first != NULL) {
		temp = m_first->getNext();
		m_first->deleteInstance();
		m_first = temp;
	}
}

//----------------------------------------------------------------------------
void GameMessageParser::addArgType(GameMessageArgumentDataType type, Int argCount) 
{
	if (m_first == NULL) {
		m_first = newInstance(GameMessageParserArgumentType)(type, argCount);
		m_last = m_first;
		return;
	}

	m_last->setNext(newInstance(GameMessageParserArgumentType)(type, argCount));
	m_last = m_last->getNext();
}

//----------------------------------------------------------------------------
GameMessageParserArgumentType::GameMessageParserArgumentType(GameMessageArgumentDataType type, Int argCount) 
{
	m_next = NULL;
	m_type = type;
	m_argCount = argCount;
}

//----------------------------------------------------------------------------
GameMessageParserArgumentType::~GameMessageParserArgumentType() 
{
}

