/* SPDX-License-Identifier: MIT OR Apache-2.0
 *
 * Headers bindgen reads, plus non-inline wrappers (csrc/shim.c) for the parts of the
 * DPDK fast path that are `static inline` in the headers (bindgen cannot link those)
 * or macros (rte_errno). Every wrapper is prefixed `wpg_` and does nothing but call
 * the DPDK function of the same name.
 *
 * `rte_config.h` is force-included by the cflags pkg-config gives (`-include`).
 */
#ifndef WPG_DPDK_SHIM_H
#define WPG_DPDK_SHIM_H

#include <stdint.h>

#include <rte_build_config.h>
#include <rte_eal.h>
#include <rte_errno.h>
#include <rte_ethdev.h>
#include <rte_launch.h>
#include <rte_lcore.h>
#include <rte_mbuf.h>
#include <rte_mempool.h>
#include <rte_version.h>

/* rte_errno is a per-lcore (thread-local) macro. */
int wpg_rte_errno(void);

/* lcore */
unsigned wpg_rte_lcore_id(void);

/* mbuf */
struct rte_mbuf *wpg_rte_pktmbuf_alloc(struct rte_mempool *mp);
int wpg_rte_pktmbuf_alloc_bulk(struct rte_mempool *mp, struct rte_mbuf **mbufs, unsigned count);
void wpg_rte_pktmbuf_free(struct rte_mbuf *m);
void wpg_rte_pktmbuf_reset(struct rte_mbuf *m);
char *wpg_rte_pktmbuf_append(struct rte_mbuf *m, uint16_t len);
void *wpg_rte_pktmbuf_mtod(struct rte_mbuf *m);
uint16_t wpg_rte_pktmbuf_data_len(const struct rte_mbuf *m);
uint32_t wpg_rte_pktmbuf_pkt_len(const struct rte_mbuf *m);
uint16_t wpg_rte_pktmbuf_tailroom(const struct rte_mbuf *m);
/* Resets `m` and copies `len` bytes into it; 0 on success, -1 if it does not fit. */
int wpg_rte_pktmbuf_copy_in(struct rte_mbuf *m, const void *data, uint16_t len);

/* ethdev fast path */
uint16_t wpg_rte_eth_tx_burst(uint16_t port_id, uint16_t queue_id, struct rte_mbuf **tx_pkts,
                              uint16_t nb_pkts);
uint16_t wpg_rte_eth_rx_burst(uint16_t port_id, uint16_t queue_id, struct rte_mbuf **rx_pkts,
                              uint16_t nb_pkts);

#endif
