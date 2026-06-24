def consume_orders(consumer):
    consumer.subscribe(["orders.created"])
