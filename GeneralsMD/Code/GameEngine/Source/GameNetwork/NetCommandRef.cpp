#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "GameNetwork/NetCommandRef.h"

#ifdef DEBUG_NETCOMMANDREF
static UnsignedInt refNum = 0;
#endif

/**
 * Constructor.  Attach to the given network command.
 */
#ifdef DEBUG_NETCOMMANDREF
NetCommandRef::NetCommandRef(NetCommandMsg *msg, char *filename, int line)
#else
NetCommandRef::NetCommandRef(NetCommandMsg *msg)
#endif
{
	m_msg = msg;
	m_next = NULL;
	m_prev = NULL;
	m_msg->attach();
	m_timeLastSent = -1;

#ifdef DEBUG_NETCOMMANDREF
	m_id = ++refNum;
	DEBUG_LOG(("NetCommandRef %d allocated in file %s line %d\n", m_id, filename, line));
#endif
}

/**
 * Destructor. Detach from the network command.
 */
NetCommandRef::~NetCommandRef() 
{
	if (m_msg != NULL) 
	{
		m_msg->detach();
	}
 	DEBUG_ASSERTCRASH(m_next == NULL, ("NetCommandRef::~NetCommandRef - m_next != NULL"));
	DEBUG_ASSERTCRASH(m_prev == NULL, ("NetCommandRef::~NetCommandRef - m_prev != NULL"));

#ifdef DEBUG_NETCOMMANDREF
	DEBUG_LOG(("NetCommandRef %d deleted\n", m_id));
#endif
}

