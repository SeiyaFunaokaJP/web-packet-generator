/* SPDX-License-Identifier: MIT OR Apache-2.0
 *
 * Out-of-line wrappers for DPDK's inline fast-path API; see shim.h. Built by build.rs
 * with the cflags pkg-config reports for libdpdk (which force-include rte_config.h and
 * set -march), so the inlined code matches what DPDK itself was built for.
 */
#include <string.h>

#include "shim.h"

int wpg_rte_errno(void) { return rte_errno; }

unsigned wpg_rte_lcore_id(void) { return rte_lcore_id(); }

struct rte_mbuf *wpg_rte_pktmbuf_alloc(struct rte_mempool *mp) { return rte_pktmbuf_alloc(mp); }

int wpg_rte_pktmbuf_alloc_bulk(struct rte_mempool *mp, struct rte_mbuf **mbufs, unsigned count)
{
	return rte_pktmbuf_alloc_bulk(mp, mbufs, count);
}

void wpg_rte_pktmbuf_free(struct rte_mbuf *m) { rte_pktmbuf_free(m); }

void wpg_rte_pktmbuf_reset(struct rte_mbuf *m) { rte_pktmbuf_reset(m); }

char *wpg_rte_pktmbuf_append(struct rte_mbuf *m, uint16_t len) { return rte_pktmbuf_append(m, len); }

void *wpg_rte_pktmbuf_mtod(struct rte_mbuf *m) { return rte_pktmbuf_mtod(m, void *); }

uint16_t wpg_rte_pktmbuf_data_len(const struct rte_mbuf *m) { return rte_pktmbuf_data_len(m); }

uint32_t wpg_rte_pktmbuf_pkt_len(const struct rte_mbuf *m) { return rte_pktmbuf_pkt_len(m); }

uint16_t wpg_rte_pktmbuf_tailroom(const struct rte_mbuf *m) { return rte_pktmbuf_tailroom(m); }

int wpg_rte_pktmbuf_copy_in(struct rte_mbuf *m, const void *data, uint16_t len)
{
	char *dst;

	rte_pktmbuf_reset(m);
	dst = rte_pktmbuf_append(m, len);
	if (dst == NULL)
		return -1;
	memcpy(dst, data, len);
	return 0;
}

uint16_t wpg_rte_eth_tx_burst(uint16_t port_id, uint16_t queue_id, struct rte_mbuf **tx_pkts,
                              uint16_t nb_pkts)
{
	return rte_eth_tx_burst(port_id, queue_id, tx_pkts, nb_pkts);
}

uint16_t wpg_rte_eth_rx_burst(uint16_t port_id, uint16_t queue_id, struct rte_mbuf **rx_pkts,
                              uint16_t nb_pkts)
{
	return rte_eth_rx_burst(port_id, queue_id, rx_pkts, nb_pkts);
}
